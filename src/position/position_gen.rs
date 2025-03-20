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
    player: u8,
    w_stones: u8,
    b_stones: u8,
}

impl FlatsConfiguration {
    /// All flat board configurations, except the first two ply where walls/caps cannot be placed
    pub fn all_after_opening(num_reserves: u8) -> Vec<Self> {
        let configuration_classes: Vec<FlatsConfiguration> = (1..=2)
            .flat_map(|player| {
                (1..=num_reserves).flat_map(move |w_stones| {
                    (1..=num_reserves).map(move |b_stones| FlatsConfiguration {
                        player,
                        w_stones,
                        b_stones,
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

struct WallConfigurationData {
    start_index: BigUint,
    size: BigUint,
}

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
struct WallConfiguration {
    w_walls: u8,
    b_walls: u8,
    w_caps: u8,
    b_caps: u8,
}

type BoardConfigurations =
    BTreeMap<FlatsConfiguration, (BigUint, BTreeMap<WallConfiguration, BigUint>)>;

pub fn configs_total2(
    size: u8,
    tiles: u8,
    max_w_stones: u8,
    max_b_stones: u8,
    max_w_caps: u8,
    max_b_caps: u8,
) -> BigUint {
    let mut total = BigUint::from(1 + size * size);
    let mut position_classes: BTreeMap<
        FlatsConfiguration,
        (BigUint, BTreeMap<WallConfiguration, BigUint>),
    > = BTreeMap::new();

    // Insert start position
    let unit_wall_config = WallConfiguration {
        w_walls: 0,
        b_walls: 0,
        w_caps: 0,
        b_caps: 0,
    };
    let unit_wall_config_map: BTreeMap<WallConfiguration, BigUint> =
        [(unit_wall_config, 1u64.into())].into_iter().collect();
    position_classes.insert(
        FlatsConfiguration {
            player: 1,
            w_stones: 0,
            b_stones: 0,
        },
        (1u64.into(), unit_wall_config_map.clone()),
    );
    position_classes.insert(
        FlatsConfiguration {
            player: 2,
            w_stones: 1,
            b_stones: 0,
        },
        ((size * size).into(), unit_wall_config_map),
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

        let old_value = position_classes.insert(
            class,
            (num_flat_configurations.clone(), blocking_configurations),
        );
        total += num_flat_configurations * num_blocking_configurations;
        assert!(old_value.is_none(), "{:?} was duplicate", class);
    }

    println!(
        "Number of classes: {}, sum {}",
        position_classes.len(),
        position_classes
            .values()
            .map(
                |(num_flat_configurations, blocking_configurations)| num_flat_configurations
                    * blocking_configurations.values().sum::<BigUint>()
            )
            .sum::<BigUint>()
    );
    println!(
        "1 flat each stats: {:?}, {:?}",
        position_classes[&FlatsConfiguration {
            player: 1,
            w_stones: 1,
            b_stones: 1
        }],
        position_classes[&FlatsConfiguration {
            player: 2,
            w_stones: 1,
            b_stones: 1
        }]
    );
    // position_classes.values().sum::<BigUint>()
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
) -> BTreeMap<WallConfiguration, BigUint> {
    let mut config_classes: BTreeMap<WallConfiguration, BigUint> = BTreeMap::new();

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
                    );
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
