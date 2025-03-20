use num_traits::ToPrimitive;
use std::collections::BTreeMap;

use num_bigint::BigUint;

use super::{starting_capstones, starting_stones};

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
struct FlatsConfiguration {
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

struct FlatConfigurationData {
    start_index: BigUint,
    size: BigUint,
    num_flats_permutations: BigUint,
    blocking_configurations: BTreeMap<WallConfiguration, u128>,
}

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
struct WallConfiguration {
    w_walls: u8,
    b_walls: u8,
    w_caps: u8,
    b_caps: u8,
}

type BoardConfigurations = BTreeMap<FlatsConfiguration, FlatConfigurationData>;

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
// ) -> (BigUint, BigUint) {
//     let flats = FlatsConfiguration {
//         w_stones: w_reserves_placed,
//         b_stones: b_reserves_placed,
//         player,
//     };
//     let flat_data = configurations.get(&flats).unwrap();
//     let blocking_configs = flat_data.1.get(&WallConfiguration {
//         w_walls,
//         b_walls,
//         w_caps,
//         b_caps,
//     });
//     let start_index = flat_data.
//         let size = blocking_configs.unwrap().clone();
//     (start_index, size)
// }

pub fn configs_total2(
    size: u8,
    tiles: u8,
    max_w_stones: u8,
    max_b_stones: u8,
    max_w_caps: u8,
    max_b_caps: u8,
) -> BigUint {
    let mut total = BigUint::from(1 + size * size);
    let mut position_classes: BoardConfigurations = BTreeMap::new();

    // Insert start position
    let unit_wall_config = WallConfiguration {
        w_walls: 0,
        b_walls: 0,
        w_caps: 0,
        b_caps: 0,
    };
    let unit_wall_config_map: BTreeMap<WallConfiguration, u128> =
        [(unit_wall_config, 1u64.into())].into_iter().collect();

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

        let num_blocking_configurations = blocking_configurations.values().sum::<BigUint>();

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
                    .sum::<BigUint>()
        );
        assert_eq!(
            position_class.start_index.clone() + position_class.size.clone(),
            next_position_class.start_index.clone()
        );
    }

    let biggest = position_classes
        .iter()
        .flat_map(|(config, data)| {
            data.blocking_configurations
                .iter()
                .map(|(wall_config, wall_data)| (config.clone(), wall_config, wall_data))
        })
        .max_by_key(|(_, _, data)| *data)
        .unwrap();

    println!(
        "Biggest class: {:?} {:?}, size {}",
        biggest.0, biggest.1, biggest.2
    );

    total
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
) -> BTreeMap<WallConfiguration, u128> {
    let mut config_classes: BTreeMap<WallConfiguration, u128> = BTreeMap::new();

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
                    let old_value = config_classes.insert(wall_config, result);
                    assert!(old_value.is_none());
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
