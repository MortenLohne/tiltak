use crate::position::ExpMove;
use crate::position::Position;
use crate::tests::do_moves_and_check_validity;
use crate::tests::move_gen_generic_tests::perft_check_answers;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;

#[test]
fn move_stack_test() {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    do_moves_and_check_validity(&mut position, &["d3", "c3", "c4", "1d3<", "1c4-", "Sc4"]);

    position.generate_moves(&mut moves);
    assert_eq!(
        moves.len(),
        69 + 18,
        "Generated wrong moves on board:\n{:?}\nExpected moves: {:?}\nExpected move moves:{:?}",
        position,
        moves,
        moves
            .iter()
            .filter(|mv| matches!(mv.expand(), ExpMove::Move(_, _, _)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn respect_carry_limit_test() {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    do_moves_and_check_validity(
        &mut position,
        &[
            "c2", "c3", "d3", "b3", "c4", "1c2+", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2+", "a2",
        ],
    );
    position.generate_moves(&mut moves);
    assert!(
        moves.contains(&position.move_from_san("5c3>").unwrap()),
        "5c3> was not a legal move among {:?} on board\n{:?}",
        moves,
        position
    );

    if let Ok(mv) = position.move_from_san("6c3>") {
        assert!(
            !moves.contains(&mv),
            "6c3> was a legal move among {:?} on board\n{:?}",
            moves,
            position
        );
    }
}

#[test]
fn start_pos_perf_test() {
    let mut position = <Position<5>>::default();
    perft_check_answers(&mut position, &[1, 25, 600, 43_320, 2_999_784]);
}

#[test]
fn perf_test2() {
    let mut position = <Position<5>>::default();

    do_moves_and_check_validity(&mut position, &["d3", "c3", "c4", "1d3<", "1c4-", "Sc4"]);

    perft_check_answers(&mut position, &[1, 87, 6155, 461_800]);
}

#[test]
fn perf_test3() {
    let mut position = <Position<5>>::default();

    do_moves_and_check_validity(
        &mut position,
        &[
            "c2", "c3", "d3", "b3", "c4", "1c2+", "1d3<", "1b3>", "1c4-", "Cc2", "a1", "1c2+", "a2",
        ],
    );

    perft_check_answers(&mut position, &[1, 104, 7743, 592_645]);
}

#[test]
fn suicide_perf_test() {
    let move_strings = [
        "c4", "c2", "d2", "c3", "b2", "d3", "1d2+", "b3", "d2", "b4", "1c2+", "1b3>", "2d3<",
        "1c4-", "d4", "5c3<23", "c2", "c4", "1d4<", "d3", "1d2+", "1c3+", "Cc3", "2c4>", "1c3<",
        "d2", "c3", "1d2+", "1c3+", "1b4>", "2b3>11", "3c4-12", "d2", "c4", "b4", "c5", "1b3>",
        "1c4<", "3c3-", "e5", "e2",
    ];

    let mut position = <Position<5>>::default();
    do_moves_and_check_validity(&mut position, &move_strings);
    perft_check_answers(&mut position, &[1, 85, 11_206, 957_000]);
}
