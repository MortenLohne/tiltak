use num_bigint::BigUint;

use super::{starting_capstones, starting_stones};

// use super::{starting_stones, Position};

// impl<const S: usize> Position<S> {
//     pub fn from_int(i: BigUint) -> Self {
//         todo!()
//     }

//     pub fn count_positions() -> BigUint {
//         for white_stones in 0..starting_stones(S) {
//             for black_stones in 0..starting_stones(S) {}
//         }
//         todo!()
//     }
// }

// pub fn configs_exact(tiles: u64, w_walls: u64, b_walls: u64, w_caps: u64, b_caps: u64) -> BigUint {
//     if w_walls + b_walls + w_caps + b_caps > tiles {
//         return 0u64.into();
//     } else {
//         return choose_multinomial(
//             tiles,
//             &[
//                 tiles - w_walls - b_walls - w_caps - b_caps,
//                 w_walls,
//                 b_walls,
//                 w_caps,
//                 b_caps,
//             ],
//         );
//     }
// }

// pub fn configs_max(
//     tiles: u64,
//     max_w_walls: u64,
//     max_b_walls: u64,
//     max_w_caps: u64,
//     max_b_caps: u64,
// ) -> BigUint {
//     let mut total = 0u64.into();
//     for w_walls in 0..=max_w_walls {
//         // TODO: Maybe only iterate up to?
//         for b_walls in 0..=max_b_walls {
//             for w_caps in 0..=max_w_caps {
//                 for b_caps in 0..=max_b_caps {
//                     total += configs_exact(tiles, w_walls, b_walls, w_caps, b_caps);
//                 }
//             }
//         }
//     }
//     total
// }

// pub fn configs_total(
//     tiles: u64,
//     max_w_stones: u64,
//     max_b_stones: u64,
//     max_w_caps: u64,
//     max_b_caps: u64,
// ) -> BigUint {
//     let mut total = 0u64.into();
//     for w_stones in 0..=max_w_stones {
//         // TODO: Maybe only iterate up to?
//         for b_stones in 0..=max_b_stones {
//             total += choose_multinomial(
//                 tiles - 1 + w_stones + b_stones,
//                 &[tiles - 1, w_stones, b_stones],
//             ) * configs_max(tiles, w_stones, b_stones, max_w_caps, max_b_caps);
//         }
//     }
//     total
// }

pub fn legal_positions(size: u64) -> BigUint {
    let num_reserves = starting_stones(size as usize) as u64;
    let num_caps = starting_capstones(size as usize) as u64;
    configs_total2(
        size,
        size * size,
        num_reserves,
        num_reserves,
        num_caps,
        num_caps,
    )
}

pub fn configs_total2(
    size: u64,
    tiles: u64,
    max_w_stones: u64,
    max_b_stones: u64,
    max_w_caps: u64,
    max_b_caps: u64,
) -> BigUint {
    let mut total = BigUint::from(1 + size * size);

    for player in 1..=2 {
        for w_stones in 1..=max_w_stones {
            for b_stones in 1..=max_b_stones {
                let mut inner_total = BigUint::default();
                for w_wall in 0..=(max_w_stones - w_stones) {
                    for b_wall in 0..=(max_b_stones - b_stones) {
                        for w_cap in 0..=max_w_caps {
                            for b_cap in 0..=max_b_caps {
                                if (w_wall + b_wall + w_cap + b_cap > tiles)
                                    || (player == 1
                                        && b_stones + b_wall + b_cap == max_b_stones + max_b_caps)
                                    || (player == 2
                                        && w_stones + w_wall + w_cap == max_w_stones + max_w_caps)
                                {
                                    continue;
                                } else {
                                    inner_total += choose_multinomial(
                                        tiles,
                                        &[
                                            tiles - w_wall - b_wall - w_cap - b_cap,
                                            w_wall,
                                            b_wall,
                                            w_cap,
                                            b_cap,
                                        ],
                                    );
                                }
                            }
                        }
                    }
                }
                total += choose_multinomial(
                    tiles - 1 + w_stones + b_stones,
                    &[tiles - 1, w_stones, b_stones],
                ) * inner_total;
            }
        }
    }

    total
}

pub fn choose_multinomial(n: u64, ks: &[u64]) -> BigUint {
    factorial(n) / ks.iter().map(|&k| factorial(k)).product::<BigUint>()
}

pub fn factorial(n: u64) -> BigUint {
    (1..=n).product()
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
