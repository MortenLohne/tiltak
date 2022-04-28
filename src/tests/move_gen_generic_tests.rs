use crate::position::Position;
use board_game_traits::Position as PositionTrait;

#[test]
fn start_position_move_gen_test() {
    start_position_move_gen_prop::<4>();
    start_position_move_gen_prop::<5>();
    start_position_move_gen_prop::<6>();
    start_position_move_gen_prop::<7>();
    start_position_move_gen_prop::<8>();
}

/// Verifies the perft result of a position against a known answer
pub fn perft_check_answers<const S: usize>(position: &mut Position<S>, answers: &[u64]) {
    for (depth, &answer) in answers.iter().enumerate() {
        for mut position in position.symmetries_with_swapped_colors() {
            assert_eq!(
                position.perft(depth as u16),
                answer,
                "Wrong perft result for depth {} on\n{:?}",
                depth,
                position
            );
        }
    }
}

fn start_position_move_gen_prop<const S: usize>() {
    let mut position = <Position<S>>::default();
    let mut moves = vec![];
    position.generate_moves(&mut moves);
    assert_eq!(moves.len(), S * S);
    for mv in moves {
        let reverse_move = position.do_move(mv);
        let mut moves = vec![];
        position.generate_moves(&mut moves);
        assert_eq!(moves.len(), S * S - 1);
        position.reverse_move(reverse_move);
    }
}
