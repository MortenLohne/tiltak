use crate::position::position_gen::PositionEncoder;
use crate::position::Position;
use num_bigint::BigUint;
use num_bigint::RandBigInt;
use pgn_traits::PgnPosition;

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
    use board_game_traits::Position as _;

    use crate::position::Position;

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
    use board_game_traits::Position as _;

    use crate::position::{Move, Position, Role, Square};

    let k: BigUint = 1u64.into();
    let data = <PositionEncoder<S>>::initialize();
    let position: Position<S> = data.decode(k.clone());
    assert_eq!(k, data.encode(&position));

    let mut start_position = Position::start_position();
    start_position.do_move(Move::placement(Role::Flat, Square::from_u8(0)));

    assert_eq!(position, start_position);
}

#[test]
fn decode_encode_random_integers_3s_test() {
    decode_encode_random_integers_prop::<3>();
}

#[test]
fn decode_encode_random_integers_4s_test() {
    decode_encode_random_integers_prop::<4>();
}

#[test]
fn decode_encode_random_integers_5s_test() {
    decode_encode_random_integers_prop::<5>();
}

/// Check that converting an integer into a position and back again results in the same integer.
/// In addition to testing the implementation,
/// it also provides strong confidence that no two integers ever map to the same position
/// i.e. that the integer representation has no redundancy
#[cfg(test)]
fn decode_encode_random_integers_prop<const S: usize>() {
    let data = <PositionEncoder<S>>::initialize();

    let max_index = data.count_legal_positions();
    let mut rng = rand::thread_rng();

    for _ in 0..100_000 {
        let k = rng.gen_biguint_below(&max_index);
        let position = data.decode(k.clone());
        assert_eq!(k, data.encode(&position), "Failed for k={}", k);

        // Check converting to and from TPS, because TPS parsing involves some extra
        // correctness checks on the position
        assert_eq!(
            <Position<S>>::from_fen(&position.to_fen()).unwrap(),
            position
        );
    }
}
