use crate::{board_mod, mcts};
use board_game_traits::board::Board;
use pgn_traits::pgn::PgnBoard;

#[test]
fn win_in_two_moves_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in ["c3", "e5", "c2", "d5", "c1", "c5", "d3", "a4", "e3"].iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }
    let mut tree = mcts::Tree::new_root();
    for _ in 0..10_000 {
        tree.select(&mut board.clone());
    }
    let (mv, _score) = tree.best_move();
    assert!(
        mv == board.move_from_san("b4").unwrap()
            || mv == board.move_from_san("b5").unwrap()
            || mv == board.move_from_san("Cb4").unwrap()
            || mv == board.move_from_san("Cb5").unwrap()
    )
}

#[test]
fn black_win_in_one_move_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in [
        "c2", "b4", "d2", "c4", "b2", "c3", "d3", "b3", "1c2-", "1b3>", "1d3<", "1c4+", "d4",
        "4c3<2", "c2", "c4", "1d4<", "1b4>", "d3", "b4", "b1", "d4", "1b2-", "2a3>", "e1", "5b3+3",
        "b3", "d1", "1e1<", "a5", "e1", "b5", "1b3-", "2c4<", "1e1-",
    ]
    .iter()
    {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
    let (best_move, _score) = mcts::mcts(board.clone(), 20_000);
    assert_eq!(
        best_move,
        board.move_from_san("3b4+").unwrap(),
        "Black didn't win on board:\n{:?}",
        board
    );
}

#[test]
fn white_can_win_in_one_move_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in ["c2", "b4", "d2", "c4", "b2", "d4", "e2", "c3"].iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
    let (best_move, _score) = mcts::mcts(board.clone(), 90_000);
    assert!(
        best_move == board.move_from_san("a2").unwrap()
            || best_move == board.move_from_san("Ca2").unwrap()
    );
}

#[test]
fn black_avoid_loss_in_one_test() {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in ["c2", "b4", "d2", "c4", "b2", "d4", "e2"].iter() {
        let mv = board.move_from_san(&mv_san).unwrap();
        board.generate_moves(&mut moves);
        assert!(
            moves.contains(&mv),
            "Move {} was not among legal moves: {:?}\n{:?}",
            board.move_to_san(&mv),
            moves,
            board
        );
        board.do_move(mv);
        moves.clear();
    }
    let (best_move, _score) = mcts::mcts(board.clone(), 90_000);
    assert!(
        best_move == board.move_from_san("a2").unwrap()
            || best_move == board.move_from_san("Sa2").unwrap()
            || best_move == board.move_from_san("Ca2").unwrap(),
        "Black didn't avoid loss on board:\n{:?}",
        board
    );
}
