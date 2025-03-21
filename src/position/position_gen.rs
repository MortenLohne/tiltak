use board_game_traits::{Color, Position as PositionTrait};
use num_traits::ToPrimitive;
use pgn_traits::PgnPosition;
use std::collections::BTreeMap;

use num_bigint::BigUint;
use num_integer::Integer;

use crate::position::{Piece, Role};

use super::{squares_iterator, starting_capstones, starting_stones, Position};

pub fn legal_positions(size: u8) -> BigUint {
    let num_reserves = starting_stones(size as usize);
    let num_caps = starting_capstones(size as usize);
    configs_total2(
        size,
        size * size,
        num_reserves,
        num_reserves,
        num_caps,
        num_caps,
    )
    .values()
    .map(|data| data.size.clone())
    .sum()
}

/// Number of board configuarions with exactly this number of white and black flats
pub fn flat_configs_exact(tiles: u64, w_stones: u64, b_stones: u64) -> BigUint {
    choose_multinomial(
        tiles - 1 + w_stones + b_stones,
        &[tiles - 1, w_stones, b_stones],
    )
}

/// Number of board configuarion with *up to* this number of white and black flats
pub fn flat_configs_max(tiles: u64, max_w_stones: u64, max_b_stones: u64) -> BigUint {
    (0..=max_w_stones)
        .flat_map(|w_stones| {
            (0..=max_b_stones).map(move |b_stones| flat_configs_exact(tiles, w_stones, b_stones))
        })
        .sum()
}

pub fn blocking_configs_exact(
    tiles: u64,
    w_walls: u64,
    b_walls: u64,
    w_caps: u64,
    b_caps: u64,
) -> BigUint {
    if w_walls + b_walls + w_caps + b_caps > tiles {
        BigUint::default()
    } else {
        choose_multinomial(
            tiles,
            &[
                tiles - w_walls - b_walls - w_caps - b_caps,
                w_walls,
                b_walls,
                w_caps,
                b_caps,
            ],
        )
    }
}

pub fn blocking_configs_max(
    tiles: u64,
    max_w_walls: u64,
    max_b_walls: u64,
    max_w_caps: u64,
    max_b_caps: u64,
) -> BigUint {
    (0..=max_w_walls)
        .flat_map(|w_walls| {
            (0..=max_b_walls).flat_map(move |b_walls| {
                (0..=max_w_caps).flat_map(move |w_caps| {
                    (0..=max_b_caps).map(move |b_caps| {
                        blocking_configs_exact(tiles, w_walls, b_walls, w_caps, b_caps)
                    })
                })
            })
        })
        .sum()
}

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
pub struct FlatsConfiguration {
    w_stones: u8,
    b_stones: u8,
    player: u8,
}

impl FlatsConfiguration {
    /// All flat board configurations, except the first two ply where walls/caps cannot be placed
    pub fn all_after_opening(num_reserves: u8) -> Vec<Self> {
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

pub struct FlatConfigurationData {
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

pub type BoardConfigurations = BTreeMap<FlatsConfiguration, FlatConfigurationData>;

pub fn max_index(configurations: &BoardConfigurations) -> BigUint {
    configurations
        .values()
        .map(|data| data.start_index.clone() + data.size.clone())
        .max()
        .unwrap()
}

fn lookup_multiset(mut k: BigUint, mut categories: Vec<u64>) -> Vec<u8> {
    let mut output: Vec<u8> = vec![];
    let num_elements = categories.iter().sum::<u64>();
    'outer: while output.len() < num_elements as usize {
        let num_elements_left = categories.iter().sum::<u64>();

        let mut ns = vec![];
        for i in 0..categories.len() {
            if categories[i] == 0 {
                ns.push(BigUint::default());
            } else {
                categories[i] -= 1;
                let n = choose_multinomial(num_elements_left - 1, &categories);
                ns.push(n);
                categories[i] += 1;
            }
        }

        let mut cumulative_sum = BigUint::default();

        for (i, n) in ns.iter().enumerate() {
            if i == ns.len() - 1 || k < cumulative_sum.clone() + n {
                assert!(
                    categories[i] > 0,
                    "Should have found a category to remove, k {}, categories {:?}, output {:?}",
                    k,
                    categories,
                    output
                );
                output.push(i as u8);
                categories[i] -= 1;
                k -= cumulative_sum;
                continue 'outer;
            }
            cumulative_sum += n;
        }
        unreachable!(
            "Should have found a category to remove, k {}, categories {:?}, output {:?}",
            k, categories, output
        );
    }
    output
}

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
fn permutation_to_index(permutation: &[u8], categories: Vec<u64>) -> BigUint {
    let mut index = BigUint::default();
    let mut categories = categories.clone();

    for &perm in permutation {
        // Count permutations that start with a "smaller" character
        for i in 0..perm {
            if categories[i as usize] > 0 {
                let mut reduced_categories = categories.clone();
                reduced_categories[i as usize] -= 1;
                index += choose_multinomial(reduced_categories.iter().sum(), &reduced_categories);
            }
        }

        categories[perm as usize] -= 1;
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
    println!("Walls permutations: {:?}", permutations);
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
    }
}

pub fn decode_position<const S: usize>(
    configurations: &BoardConfigurations,
    k: BigUint,
) -> Position<S> {
    println!("Decoding {} for {}s", k, S);
    let (flat_config, flat_data) = configurations
        .iter()
        .find(|(_, data)| k >= data.start_index && k < data.start_index.clone() + data.size.clone())
        .unwrap();
    println!(
        "Found flat configuration in the {}-{} index range",
        flat_data.start_index,
        flat_data.start_index.clone() + flat_data.size.clone()
    );
    let local_index = k - flat_data.start_index.clone();
    println!("Local index: {}", local_index);

    let side_to_move = if flat_config.player == 1 {
        Color::White
    } else {
        Color::Black
    };

    println!(
        "Determined {} to move, {} white flats and {} black flats, {} flat configurations, {} total size for this configuration",
        side_to_move,
        flat_config.w_stones,
        flat_config.b_stones,
        flat_data.num_flats_permutations,
        flat_data.size
    );
    let (walls_local_k, flat_identifier) = local_index.div_rem(&flat_data.num_flats_permutations);
    println!(
        "Extracted k={} for the flats, and k={} for the wall configuration",
        flat_identifier, walls_local_k
    );

    let (wall_config, wall_size) = flat_data
        .blocking_configurations
        .iter()
        .find(|(_config, data)| {
            walls_local_k >= data.start_index
                && walls_local_k < data.start_index.clone() + data.size
        })
        .unwrap();

    println!(
        "Found wall configuration in the {}-{} index range",
        wall_size.start_index,
        wall_size.start_index.clone() + wall_size.size
    );

    println!(
        "Determined {} white walls, {} black walls, {} white caps, {} black caps",
        wall_config.w_walls, wall_config.b_walls, wall_config.w_caps, wall_config.b_caps
    );

    let mut position: Position<S> = decode_flats(
        flat_identifier,
        flat_config.w_stones as u64,
        flat_config.b_stones as u64,
    );
    if side_to_move == Color::Black {
        position.null_move();
    }

    let walls_local_index = walls_local_k - wall_size.start_index.clone();

    println!("Got flats position: {}", position.to_fen());
    println!("Extracted local index for walls: {}", walls_local_index);

    decode_walls_caps(
        walls_local_index,
        &mut position,
        wall_config.w_walls as u64,
        wall_config.b_walls as u64,
        wall_config.w_caps as u64,
        wall_config.b_caps as u64,
    );

    println!("Got full position: {}", position.to_fen());

    position
}

/// Return the starting index and the number of positions for the given configuration
// fn lookup(
//     configurations: &BoardConfigurations,
//     w_reserves_placed: u8,
//     b_reserves_placed: u8,
//     player: u8,
//     w_walls: u8,
//     b_walls: u8,
//     w_caps: u8,
//     b_caps: u8,
// ) -> (BigUint, u128) {
//     let flats = FlatsConfiguration {
//         w_stones: w_reserves_placed,
//         b_stones: b_reserves_placed,
//         player,
//     };
//     let flat_data = configurations.get(&flats).unwrap();
//     let blocking_configs = flat_data.blocking_configurations.get(&WallConfiguration {
//         w_walls,
//         b_walls,
//         w_caps,
//         b_caps,
//     });
//     let start_index = flat_data.start_index.clone();
//     let size = blocking_configs.unwrap().clone();
//     println!("")
//     (start_index, size)
// }

pub fn configs_total2(
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
            w_stones: 1,
            b_stones: 0,
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

    println!(
        "Number of classes: {}, sum {}",
        position_classes.len(),
        position_classes
            .values()
            .map(|flat_configuration_data| flat_configuration_data.size.clone())
            .sum::<BigUint>()
    );

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

    let biggest_flat = position_classes
        .iter()
        .max_by_key(|(_, data)| data.size.clone())
        .unwrap();

    let biggest_wall = position_classes
        .iter()
        .flat_map(|(config, data)| {
            data.blocking_configurations
                .iter()
                .map(|(wall_config, wall_data)| (config.clone(), wall_config, wall_data))
        })
        .max_by_key(|(_, _, data)| data.size)
        .unwrap();

    println!(
        "Biggest flat class: {:?}, flat permutations: {}, total size {}",
        biggest_flat.0, biggest_flat.1.num_flats_permutations, biggest_flat.1.size
    );
    println!(
        "Biggest wall class: {:?} {:?}, size {}",
        biggest_wall.0, biggest_wall.1, biggest_wall.2.size
    );

    println!(
        "Looking up multiset k=10, a=2, b=1, c=1: {:?}",
        lookup_multiset(0u64.into(), vec![2, 1, 1])
    );
    println!(
        "Looking up multiset k=10, a=2, b=1, c=1: {:?}",
        kth_permutation(0u64.into(), vec![2, 1, 1])
    );

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
                        || (player == 1 && b_stones + b_wall + b_cap == max_b_stones + max_b_caps)
                        || (player == 2 && w_stones + w_wall + w_cap == max_w_stones + max_w_caps)
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

pub fn choose_multinomial(n: u64, ks: &[u64]) -> BigUint {
    factorial(n) / ks.iter().map(|&k| factorial(k)).product::<BigUint>()
}

pub fn factorial(n: u64) -> BigUint {
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
    assert_eq!(legal_positions(3), "96317109784544".parse().unwrap());
}

#[test]
fn count_positions_4s() {
    assert_eq!(
        legal_positions(4),
        "186068001400694400221565".parse().unwrap()
    );
}

#[test]
fn count_positions_5s() {
    assert_eq!(
        legal_positions(5),
        "17373764696009420300241450342663955626".parse().unwrap()
    );
}

#[test]
fn count_positions_6s() {
    assert_eq!(
        legal_positions(6),
        "234953877228339135776421063941057364108851372312359713"
            .parse()
            .unwrap()
    );
}

#[test]
fn configuration_classes_are_sorted_test() {
    let classes = FlatsConfiguration::all_after_opening(10);
    assert!(classes.is_sorted());
}
