use board_game_traits::{Color, Position as PositionTrait};
use num_traits::ToPrimitive;
use std::collections::BTreeMap;

use num_bigint::BigUint;
use num_integer::Integer;

use crate::position::{Piece, Role};

use super::{squares_iterator, starting_capstones, starting_stones, Position};

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
struct FlatsConfiguration {
    w_stones: u8,
    b_stones: u8,
    player: u8,
}

impl FlatsConfiguration {
    /// All flat board configurations, except the first two ply where walls/caps cannot be placed
    fn all_after_opening(num_reserves: u8) -> Vec<Self> {
        let configuration_classes: Vec<FlatsConfiguration> = (1..=num_reserves)
            .flat_map(move |w_stones| {
                (1..=num_reserves).flat_map(move |b_stones| {
                    (1..=2).map(move |player| FlatsConfiguration {
                        w_stones,
                        b_stones,
                        player,
                    })
                })
            })
            .collect();
        assert!(configuration_classes.is_sorted());
        configuration_classes
    }
}

struct FlatConfigurationData {
    start_index: BigUint,
    size: BigUint,
    num_flats_permutations: BigUint,
    blocking_configurations: BTreeMap<WallConfiguration, WallConfigurationData>,
}

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
struct WallConfiguration {
    w_walls: u8,
    b_walls: u8,
    w_caps: u8,
    b_caps: u8,
}

#[derive(Clone, Debug)]
struct WallConfigurationData {
    start_index: BigUint, // This is the start index relative to the flat configuration context that the wall configuration is in
    size: u128,
}

/// A struct with a pre-computed index required to encode and decode positions
pub struct PositionEncoder<const S: usize> {
    data: BoardConfigurations,
}

impl<const S: usize> PositionEncoder<S> {
    /// Initialize the encoder.
    /// This is an expensive and memory-heavy operation, especially for larger board sizes, so re-use this data structure when possible.
    pub fn initialize() -> Self {
        if !(3..=8).contains(&S) {
            panic!("Unsupported board size {}", S);
        }
        let starting_reserves = starting_stones(S);
        let starting_caps = starting_capstones(S);
        Self {
            data: configs_total2(
                S as u8,
                (S * S) as u8,
                starting_reserves,
                starting_reserves,
                starting_caps,
                starting_caps,
            ),
        }
    }

    pub fn count_legal_positions(&self) -> BigUint {
        let last_flat_data = self.data.last_key_value().unwrap().1;
        last_flat_data.start_index.clone() + last_flat_data.size.clone()
    }

    pub fn encode(&self, position: &Position<S>) -> BigUint {
        let group_data = position.group_data();
        let flat_config = FlatsConfiguration {
            player: match position.side_to_move() {
                Color::White => 1,
                Color::Black => 2,
            },
            w_stones: starting_stones(S)
                - position.white_reserves_left() as u8
                - group_data.white_walls.count(),
            b_stones: starting_stones(S)
                - position.black_reserves_left() as u8
                - group_data.black_walls.count(),
        };

        let wall_config = WallConfiguration {
            w_walls: group_data.white_walls.count(),
            b_walls: group_data.black_walls.count(),
            w_caps: group_data.white_caps.count(),
            b_caps: group_data.black_caps.count(),
        };

        let flat_data = self.data.get(&flat_config).unwrap();
        let wall_data = flat_data.blocking_configurations.get(&wall_config).unwrap();

        let flat_permutation = encode_flats(position);
        let wall_permutation = encode_walls_caps(position);

        let flat_index = flat_permutation;
        let wall_index = flat_data.num_flats_permutations.clone()
            * (wall_data.start_index.clone() + wall_permutation);

        flat_data.start_index.clone() + flat_index + wall_index
    }

    /// Decodes a position
    /// The move counter is guessed on a best-effort basis, as it is not stored in the encoded data
    ///
    /// # Panics
    /// Panics if the input is greater than or equal to the total number of positions
    pub fn decode(&self, k: BigUint) -> Position<S> {
        let (flat_config, flat_data) = self
            .data
            .iter()
            .find(|(_, data)| {
                k >= data.start_index && k < data.start_index.clone() + data.size.clone()
            })
            .expect("Tried to decode an integer greater than the total number of positions");

        let local_index = k.clone() - flat_data.start_index.clone();

        let side_to_move = match flat_config.player {
            1 => Color::White,
            2 => Color::Black,
            _ => panic!(),
        };

        let (walls_local_k, flat_identifier) =
            local_index.div_rem(&flat_data.num_flats_permutations);

        let (wall_config, wall_size) = flat_data
            .blocking_configurations
            .iter()
            .find(|(_config, data)| {
                walls_local_k >= data.start_index
                    && walls_local_k < data.start_index.clone() + data.size
            })
            .unwrap();

        let mut position: Position<S> = decode_flats(
            flat_identifier.clone(),
            flat_config.w_stones as u64,
            flat_config.b_stones as u64,
        );
        if side_to_move == Color::Black {
            position.null_move();
        }

        debug_assert_eq!(flat_identifier, encode_flats(&position));

        let walls_local_index = walls_local_k - wall_size.start_index.clone();

        decode_walls_caps(
            walls_local_index.clone(),
            &mut position,
            wall_config.w_walls as u64,
            wall_config.b_walls as u64,
            wall_config.w_caps as u64,
            wall_config.b_caps as u64,
        );

        debug_assert_eq!(walls_local_index, encode_walls_caps(&position));

        debug_assert_eq!(k, self.encode(&position));

        position
    }
}

type BoardConfigurations = BTreeMap<FlatsConfiguration, FlatConfigurationData>;

/// Find the k-th permutation of a multiset with arbitrary category counts.
fn kth_permutation(mut k: BigUint, mut categories: Vec<u64>) -> Vec<u8> {
    let mut permutation = Vec::new();

    while categories.iter().sum::<u64>() > 0 {
        for (i, &count) in categories.iter().enumerate() {
            if count == 0 {
                continue;
            }

            // Compute number of permutations starting with this category
            let mut reduced_categories = categories.clone();
            reduced_categories[i] -= 1;
            let num_permutations =
                choose_multinomial(reduced_categories.iter().sum(), &reduced_categories);

            if k < num_permutations {
                // Choose this category for the next position
                permutation.push(i as u8);
                categories[i] -= 1;
                break;
            } else {
                // Skip this many permutations and move to the next category
                k -= num_permutations;
            }
        }
    }
    permutation
}

/// Compute the lexicographic index of a given permutation.
fn permutation_to_index(permutation: &[u8], mut categories: Vec<u64>) -> BigUint {
    let mut index = BigUint::default();

    for &perm in permutation {
        // Count permutations that start with a "smaller" character
        for i in 0..perm {
            if categories[i as usize] > 0 {
                let mut reduced_categories = categories.clone();
                reduced_categories[i as usize] -= 1;
                index += choose_multinomial(reduced_categories.iter().sum(), &reduced_categories);
            }
        }

        categories[perm as usize] = categories[perm as usize].overflowing_sub(1).0;
        // Overflow only happens on the last loop, so it doesn't matter
    }

    index
}

fn decode_flats<const S: usize>(
    k: BigUint,
    num_white_flats: u64,
    num_black_flats: u64,
) -> Position<S> {
    let permutations = kth_permutation(
        k,
        vec![num_white_flats, num_black_flats, (S * S) as u64 - 1],
    );
    let mut position = Position::default();
    let mut permutations_iter = permutations.into_iter();
    'square_loop: for square in squares_iterator::<S>() {
        loop {
            let Some(next) = permutations_iter.next() else {
                return position;
            };
            if next == 2 {
                continue 'square_loop;
            }
            let piece = match next {
                0 => Piece::WhiteFlat,
                1 => Piece::BlackFlat,
                _ => unreachable!(),
            };
            let mut stack = position.get_stack(square);
            stack.push(piece);
            position.set_stack(square, stack);
            position.half_moves_played += 1;
            match piece {
                Piece::WhiteFlat => position.white_stones_left -= 1,
                Piece::BlackFlat => position.black_stones_left -= 1,
                _ => unreachable!(),
            }
        }
    }
    unreachable!()
}

fn decode_walls_caps<const S: usize>(
    k: BigUint,
    position: &mut Position<S>,
    num_white_walls: u64,
    num_black_walls: u64,
    num_white_caps: u64,
    num_black_caps: u64,
) {
    let permutations = kth_permutation(
        k,
        vec![
            num_white_walls,
            num_black_walls,
            num_white_caps,
            num_black_caps,
            (S * S) as u64 - num_white_walls - num_black_walls - num_white_caps - num_black_caps,
        ],
    );

    let mut permutations_iter = permutations.into_iter();
    'square_loop: for square in squares_iterator::<S>() {
        let next = permutations_iter.next().unwrap();
        if next == 4 {
            continue 'square_loop;
        }
        let piece = match next {
            0 => Piece::WhiteWall,
            1 => Piece::BlackWall,
            2 => Piece::WhiteCap,
            3 => Piece::BlackCap,
            _ => unreachable!(),
        };
        let mut stack = position.get_stack(square);
        assert!(stack
            .top_stone()
            .is_none_or(|piece| piece.role() == Role::Flat));
        stack.push(piece);
        position.set_stack(square, stack);
        position.half_moves_played += 1;
        match piece {
            Piece::WhiteWall => position.white_stones_left -= 1,
            Piece::BlackWall => position.black_stones_left -= 1,
            Piece::WhiteCap => position.white_caps_left -= 1,
            Piece::BlackCap => position.black_caps_left -= 1,
            _ => unreachable!(),
        }
    }
}

fn encode_flats<const S: usize>(position: &Position<S>) -> BigUint {
    let group_data = position.group_data();
    let flat_config = FlatsConfiguration {
        player: match position.side_to_move() {
            Color::White => 1,
            Color::Black => 2,
        },
        w_stones: starting_stones(S)
            - position.white_reserves_left() as u8
            - group_data.white_walls.count(),
        b_stones: starting_stones(S)
            - position.black_reserves_left() as u8
            - group_data.black_walls.count(),
    };

    let mut permutation = vec![];
    for square in squares_iterator::<S>() {
        let stack = position.get_stack(square);
        for piece in stack {
            match piece {
                Piece::WhiteFlat => permutation.push(0),
                Piece::BlackFlat => permutation.push(1),
                _ => (),
            }
        }
        permutation.push(2);
    }

    let categories = vec![
        flat_config.w_stones as u64,
        flat_config.b_stones as u64,
        (S * S) as u64 - 1,
    ];

    let kth = permutation_to_index(&permutation, categories);
    kth
}

fn encode_walls_caps<const S: usize>(position: &Position<S>) -> BigUint {
    let group_data = position.group_data();
    let wall_config = WallConfiguration {
        w_walls: group_data.white_walls.count(),
        b_walls: group_data.black_walls.count(),
        w_caps: group_data.white_caps.count(),
        b_caps: group_data.black_caps.count(),
    };

    let mut permutation = vec![];
    for square in squares_iterator::<S>() {
        match position.top_stones()[square] {
            Some(Piece::WhiteWall) => permutation.push(0),
            Some(Piece::BlackWall) => permutation.push(1),
            Some(Piece::WhiteCap) => permutation.push(2),
            Some(Piece::BlackCap) => permutation.push(3),
            _ => permutation.push(4),
        }
    }

    let kth = permutation_to_index(
        &permutation,
        vec![
            wall_config.w_walls as u64,
            wall_config.b_walls as u64,
            wall_config.w_caps as u64,
            wall_config.b_caps as u64,
            (S * S) as u64
                - wall_config.w_walls as u64
                - wall_config.b_walls as u64
                - wall_config.w_caps as u64
                - wall_config.b_caps as u64,
        ],
    );
    kth
}

fn configs_total2(
    size: u8,
    tiles: u8,
    max_w_stones: u8,
    max_b_stones: u8,
    max_w_caps: u8,
    max_b_caps: u8,
) -> BoardConfigurations {
    let mut total = BigUint::from(1 + size * size);
    let mut position_classes: BoardConfigurations = BTreeMap::new();

    // Insert start position
    let unit_wall_config = WallConfiguration {
        w_walls: 0,
        b_walls: 0,
        w_caps: 0,
        b_caps: 0,
    };
    let unit_wall_config_map: BTreeMap<WallConfiguration, WallConfigurationData> = [(
        unit_wall_config,
        WallConfigurationData {
            start_index: 0u64.into(),
            size: 1,
        },
    )]
    .into_iter()
    .collect();

    position_classes.insert(
        FlatsConfiguration {
            player: 1,
            w_stones: 0,
            b_stones: 0,
        },
        FlatConfigurationData {
            start_index: 0u64.into(),
            size: 1u64.into(),
            num_flats_permutations: 1u64.into(),
            blocking_configurations: unit_wall_config_map.clone(),
        },
    );

    position_classes.insert(
        FlatsConfiguration {
            player: 2,
            w_stones: 0,
            b_stones: 1,
        },
        FlatConfigurationData {
            start_index: 1u64.into(),
            size: (size * size).into(),
            num_flats_permutations: (size * size).into(),
            blocking_configurations: unit_wall_config_map,
        },
    );

    for class @ FlatsConfiguration {
        player,
        w_stones,
        b_stones,
    } in FlatsConfiguration::all_after_opening(max_w_stones)
    {
        let blocking_configurations = inner_configs(
            tiles,
            max_w_stones,
            max_b_stones,
            max_w_caps,
            max_b_caps,
            player,
            w_stones,
            b_stones,
        );

        let num_blocking_configurations = blocking_configurations
            .values()
            .map(|data| data.size)
            .sum::<BigUint>();

        let num_flat_configurations = choose_multinomial(
            (tiles - 1 + w_stones + b_stones) as u64,
            &[tiles as u64 - 1, w_stones as u64, b_stones as u64],
        );

        let flat_config_data = FlatConfigurationData {
            start_index: total.clone(),
            size: num_flat_configurations.clone() * num_blocking_configurations.clone(),
            num_flats_permutations: num_flat_configurations.clone(),
            blocking_configurations,
        };

        let old_value = position_classes.insert(class, flat_config_data);
        assert!(old_value.is_none(), "{:?} was duplicate", class);
        assert_eq!(
            total,
            position_classes.last_entry().unwrap().get().start_index
        );

        total += num_flat_configurations * num_blocking_configurations;
    }

    let final_configuration = position_classes.last_key_value().as_ref().unwrap().1;
    assert_eq!(
        total,
        final_configuration.start_index.clone() + final_configuration.size.clone()
    );
    for (position_class, next_position_class) in position_classes
        .values()
        .zip(position_classes.values().skip(1))
    {
        assert_eq!(
            position_class.size,
            position_class.num_flats_permutations.clone()
                * position_class
                    .blocking_configurations
                    .values()
                    .map(|data| data.size)
                    .sum::<BigUint>()
        );
        assert_eq!(
            position_class.start_index.clone() + position_class.size.clone(),
            next_position_class.start_index.clone()
        );
    }

    for position_class in position_classes.values() {
        for (wall_data, next_wall_data) in position_class
            .blocking_configurations
            .values()
            .zip(position_class.blocking_configurations.values().skip(1))
        {
            assert_eq!(
                wall_data.start_index.clone() + wall_data.size.clone(),
                next_wall_data.start_index.clone()
            );
        }
    }

    position_classes
}

fn inner_configs(
    tiles: u8,
    max_w_stones: u8,
    max_b_stones: u8,
    max_w_caps: u8,
    max_b_caps: u8,
    player: u8,
    w_stones: u8,
    b_stones: u8,
) -> BTreeMap<WallConfiguration, WallConfigurationData> {
    let mut config_classes: BTreeMap<WallConfiguration, WallConfigurationData> = BTreeMap::new();
    let mut total = BigUint::default();

    for w_wall in 0..=(max_w_stones - w_stones) {
        for b_wall in 0..=(max_b_stones - b_stones) {
            for w_cap in 0..=max_w_caps {
                for b_cap in 0..=max_b_caps {
                    if (w_wall + b_wall + w_cap + b_cap > tiles)
                        || (player == 2 && b_stones + b_wall + b_cap == max_b_stones + max_b_caps)
                        || (player == 1 && w_stones + w_wall + w_cap == max_w_stones + max_w_caps)
                    {
                        continue;
                    }
                    let result = choose_multinomial(
                        tiles as u64,
                        &[
                            (tiles - w_wall - b_wall - w_cap - b_cap) as u64,
                            w_wall as u64,
                            b_wall as u64,
                            w_cap as u64,
                            b_cap as u64,
                        ],
                    )
                    .to_u128()
                    .unwrap();
                    let wall_config = WallConfiguration {
                        w_walls: w_wall,
                        b_walls: b_wall,
                        w_caps: w_cap,
                        b_caps: b_cap,
                    };
                    let wall_data: WallConfigurationData = WallConfigurationData {
                        start_index: total.clone(),
                        size: result,
                    };
                    let old_value = config_classes.insert(wall_config, wall_data);
                    assert!(old_value.is_none());

                    total += result;
                }
            }
        }
    }
    config_classes
}

fn choose_multinomial(n: u64, ks: &[u64]) -> BigUint {
    factorial(n) / ks.iter().map(|&k| factorial(k)).product::<BigUint>()
}

fn factorial(n: u64) -> BigUint {
    (1..=n).product()
}

#[test]
fn kth_permutation_inverse_test() {
    let k: BigUint = 1234u64.into();
    let categories = vec![2, 5, 3, 10];
    let permutation = kth_permutation(k.clone(), categories.clone());
    let index = permutation_to_index(&permutation, categories);
    assert_eq!(index, k);
}

#[test]
fn factorial_test() {
    assert_eq!(factorial(0), 1u64.into());
    assert_eq!(factorial(1), 1u64.into());
    assert_eq!(factorial(2), 2u64.into());
    assert_eq!(factorial(3), 6u64.into());
    assert_eq!(factorial(4), 24u64.into());
    assert_eq!(factorial(5), 120u64.into());
    assert_eq!(factorial(6), 720u64.into());
}

#[test]
fn count_positions_3s() {
    assert_eq!(
        <PositionEncoder<3>>::initialize().count_legal_positions(),
        "96317109784544".parse().unwrap()
    );
}

#[test]
fn count_positions_4s() {
    assert_eq!(
        <PositionEncoder<4>>::initialize().count_legal_positions(),
        "186068001400694400221565".parse().unwrap()
    );
}

#[test]
fn count_positions_5s() {
    assert_eq!(
        <PositionEncoder<5>>::initialize().count_legal_positions(),
        "17373764696009420300241450342663955626".parse().unwrap()
    );
}

#[test]
fn count_positions_6s() {
    assert_eq!(
        <PositionEncoder<6>>::initialize().count_legal_positions(),
        "234953877228339135776421063941057364108851372312359713"
            .parse()
            .unwrap()
    );
}

#[test]
fn decode_max_number_test() {
    let encoder = <PositionEncoder<5>>::initialize();
    encoder.decode(encoder.count_legal_positions() - BigUint::from(1u64));
}

#[test]
#[should_panic]
fn decode_too_large_number_test() {
    let encoder = <PositionEncoder<5>>::initialize();
    encoder.decode(encoder.count_legal_positions());
}

#[test]
fn encode_start_position_test() {
    encode_start_position_prop::<3>();
    encode_start_position_prop::<4>();
    encode_start_position_prop::<5>();
}

#[cfg(test)]
fn encode_start_position_prop<const S: usize>() {
    let k: BigUint = 0u64.into();
    let data = PositionEncoder::<S>::initialize();
    let position: Position<S> = data.decode(k.clone());
    assert_eq!(position, Position::start_position());
    assert_eq!(k, data.encode(&position));
}

#[test]
fn encode_2nd_ply_test() {
    encode_2nd_ply_prop::<3>();
    encode_2nd_ply_prop::<4>();
    encode_2nd_ply_prop::<5>();
}

#[cfg(test)]
fn encode_2nd_ply_prop<const S: usize>() {
    use crate::position::{Move, Square};

    let k: BigUint = 1u64.into();
    let data = <PositionEncoder<S>>::initialize();
    let position: Position<S> = data.decode(k.clone());
    assert_eq!(k, data.encode(&position));

    let mut start_position = Position::start_position();
    start_position.do_move(Move::placement(Role::Flat, Square::from_u8(0)));

    assert_eq!(position, start_position);
}

#[test]
fn configuration_classes_are_sorted_test() {
    let classes = FlatsConfiguration::all_after_opening(10);
    assert!(classes.is_sorted());
}
