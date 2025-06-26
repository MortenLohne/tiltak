use crate::position::position_gen::PositionEncoder;
use num_bigint::BigUint;

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
