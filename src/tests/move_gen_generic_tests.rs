use crate::board::Board;
use board_game_traits::board::Board as EvalBoard;

#[test]
fn start_board_move_gen_test() {
    start_board_move_gen_prop::<3>();
    start_board_move_gen_prop::<4>();
    start_board_move_gen_prop::<5>();
    start_board_move_gen_prop::<6>();
    start_board_move_gen_prop::<7>();
    start_board_move_gen_prop::<8>();
}

fn start_board_move_gen_prop<const S: usize>() {
    let mut board = <Board<S>>::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    assert_eq!(moves.len(), 25);
    for mv in moves {
        let reverse_move = board.do_move(mv);
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        assert_eq!(moves.len(), 24);
        board.reverse_move(reverse_move);
    }
}
