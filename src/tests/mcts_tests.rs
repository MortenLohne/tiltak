use crate::{board_mod, mcts};
use board_game_traits::board::Board;
use pgn_traits::pgn::PgnBoard;

#[test]
fn win_in_two_moves_test() {
    let move_strings = ["c3", "e5", "c2", "d5", "c1", "c5", "d3", "a4", "e3"];

    plays_correct_move_property(
        &move_strings,
        TacticAnswer::PlayMoves(&["b4", "b5", "Cb4", "Cb5"]),
    );
}

#[test]
fn black_win_in_one_move_test() {
    let move_strings = [
        "c2", "b4", "d2", "c4", "b2", "c3", "d3", "b3", "1c2-", "1b3>", "1d3<", "1c4+", "d4",
        "4c3<2", "c2", "c4", "1d4<", "1b4>", "d3", "b4", "b1", "d4", "1b2-", "2a3>", "e1", "5b3+3",
        "b3", "d1", "1e1<", "a5", "e1", "b5", "1b3-", "2c4<", "1e1-",
    ];

    plays_correct_move_property(&move_strings, TacticAnswer::PlayMoves(&["3b4+"]));
}

#[test]
fn white_can_win_in_one_move_test() {
    let move_strings = ["c2", "b4", "d2", "c4", "b2", "d4", "e2", "c3"];

    plays_correct_move_property(&move_strings, TacticAnswer::PlayMoves(&["a2", "Ca2"]));
}

#[test]
fn black_avoid_loss_in_one_test() {
    let move_strings = ["c2", "b4", "d2", "c4", "b2", "d4", "e2"];

    plays_correct_move_property(
        &move_strings,
        TacticAnswer::PlayMoves(&["a2", "Ca2", "Sa2"]),
    );
}

#[test]
fn black_avoid_loss_in_one_test2() {
    let move_strings = [
        "c2", "b4", "d2", "d4", "b2", "c4", "e2", "a2", "c3", "b3", "1b2-", "1c4+", "1c2-", "b2",
        "b1", "d1", "c2", "a3", "2b3+", "1a2>", "1b1-",
    ];
    plays_correct_move_property(&move_strings, TacticAnswer::PlayMoves(&["1d1-"]));
}

#[test]
fn black_avoid_loss_in_one_test3() {
    let move_strings = [
        "c2", "c3", "d2", "d3", "1d2-", "c4", "d2", "b4", "1c2-", "1c4+", "2d3<", "d4", "b2", "a5",
        "c2", "a2", "b1", "1a2>", "1b1-", "1d4+", "5c3>3",
    ];

    plays_correct_move_property(&move_strings, TacticAnswer::PlayMoves(&["Ca2", "Sa2"]));
}

#[test]
fn black_avoid_loss_in_one_test4() {
    let moves_strings = [
        "c2", "c3", "d2", "b4", "1c2-", "d4", "c2", "b2", "1c2<", "d5", "c2", "a3", "e2", "a2",
        "b1", "a1", "d3", "c4", "2c3-", "1d4<", "Cc3", "3c4>2", "1c3-", "e1", "c3", "1a1>", "d1",
        "c5", "b5", "1b4-", "1d3-",
    ];

    plays_correct_move_property(&moves_strings, TacticAnswer::AvoidMoves(&["2b5>1"]));
}

#[test]
fn black_avoid_less_in_one_test5() {
    let move_strings = [
        "c2", "b3", "d2", "c3", "b2", "d4", "1b2-", "d3", "1d2-", "1c3>", "Cc3", "b4", "1c3>",
        "d2", "2d3+", "b2", "1c2<", "1b4+", "2b2-", "c2", "3d2<", "d1", "b2", "c4", "2d3-", "1c4>",
        "e1", "c4", "b4", "3d4<2", "d2", "1d1-", "4c2>", "3b4+", "1b2-", "1d4+", "3b3+2",
    ];

    plays_correct_move_property(
        &move_strings,
        TacticAnswer::PlayMoves(&["Sb4", "Cb4", "Sb5", "Cb5", "1c4<", "2c4<"]),
    );
}

#[test]
fn do_not_suicide_as_black_test() {
    let move_strings = [
        "c2", "c4", "d2", "c3", "b2", "d3", "1d2-", "b3", "d2", "b4", "1c2-", "1b3>", "2d3<",
        "1c4+", "d4", "5c3<3", "c2", "c4", "1d4<", "d3", "1d2-", "1c3-", "Cc3", "2c4>", "1c3<",
        "d2", "c3", "1d2-", "1c3-", "1b4>", "2b3>1", "3c4+2", "d2", "c4", "b4", "c5", "1b3>",
        "1c4<", "3c3+", "e5", "e2",
    ];

    plays_correct_move_property(&move_strings, TacticAnswer::AvoidMoves(&["2a3+1"]));
}

#[test]
fn do_not_suicide_as_black_test2() {
    let move_strings = [
        "c2", "d3", "d2", "c3", "1d2-", "c4", "d2", "b3", "1c2-", "1b3>", "2d3<", "1c4+", "b2",
        "5c3-3", "c2", "d3", "1d2-", "b4", "d2", "d4", "2d3-", "b3", "1b2-", "1b4+", "d3", "b2",
        "b1", "1b2>", "1d2<", "1c3+", "c1", "3b3>2", "1c1-", "1c3+", "3d4+", "4c2<2", "1b1-",
        "2a2>", "d2", "3b2<", "1d2<", "2b2>", "d4", "d2", "5d3+4",
    ];

    plays_correct_move_property(&move_strings, TacticAnswer::AvoidMoves(&["2c5>1"]));
}

/// The correct answer to a tactic can be either a lost of winning/non-losing moves, or simply a list of moves to specifically avoid
enum TacticAnswer {
    AvoidMoves(&'static [&'static str]),
    PlayMoves(&'static [&'static str]),
}

fn plays_correct_move_property(move_strings: &[&str], correct_moves: TacticAnswer) {
    let mut board = board_mod::Board::default();
    let mut moves = vec![];

    for mv_san in move_strings.iter() {
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

    let mut mcts = mcts::Tree::new_root();

    for i in 1..50000 {
        mcts.select(&mut board.clone());
        if i % 10000 == 0 {
            let (best_move, _score) = mcts.best_move();
            match correct_moves {
                TacticAnswer::AvoidMoves(moves) =>
                    assert!(moves
                        .iter()
                        .all(|mv| best_move != board.move_from_san(mv).unwrap()),
                            "{} played {}, one of the losing moves {:?} after {} iterations on board:\n{:?}",
                            board.side_to_move(), board.move_to_san(&best_move), moves, i, board),
                TacticAnswer::PlayMoves(moves) =>
                    assert!(moves
                                .iter()
                                .any(|mv| best_move == board.move_from_san(mv).unwrap()),
                            "{} didn't play one of {:?} to avoid loss, {} played instead after {} iterations on board:\n{:?}",
                            board.side_to_move(), moves, board.move_to_san(&best_move), i, board),
            }
        }
    }
}
