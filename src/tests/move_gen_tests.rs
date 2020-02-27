use crate::board::{Move, Piece, Square, Direction, Movement};
use pgn_traits::pgn::PgnBoard;
use board_game_traits::board::Board as BoardTrait;
use crate::board::Board;

#[test]
fn start_board_move_gen_test() {
    let mut board = Board::default();
    let mut moves = vec![];
    board.generate_moves(&mut moves);
    assert_eq!(moves.len(), 75);
    for mv in moves {
        let reverse_move = board.do_move(mv);
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        assert_eq!(moves.len(), 72);
        board.reverse_move(reverse_move);
    }
}

#[test]
fn move_stack_test() {
    let mut board = Board::default();
    let mut moves = vec![];

    for mv in [
        Move::Place(Piece::WhiteFlat, Square(12)),
        Move::Place(Piece::BlackFlat, Square(13)),
        Move::Place(Piece::WhiteFlat, Square(17)),
        Move::Move(
            Square(13),
            Direction::West,
            smallvec![Movement { pieces_to_take: 1 }],
        ),
        Move::Move(
            Square(17),
            Direction::North,
            smallvec![Movement { pieces_to_take: 1 }],
        ),
        Move::Place(Piece::BlackStanding, Square(17)),
    ]
        .iter()
    {
        board.generate_moves(&mut moves);
        assert!(moves.contains(mv));
        assert_eq!(*mv, board.move_from_san(&board.move_to_san(mv)).unwrap());
        board.do_move(mv.clone());
        moves.clear();
    }
    board.generate_moves(&mut moves);
    assert_eq!(
        moves.len(),
        69 + 18,
        "Generated wrong moves on board:\n{:?}\nExpected moves: {:?}\nExpected move moves:{:?}",
        board,
        moves,
        moves
            .iter()
            .filter(|mv| match mv {
                Move::Move(_, _, _) => true,
                _ => false,
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn respect_carry_limit_test() {
    let mut board = Board::default();
    let mut moves = vec![];

    for move_string in [
        "c3", "c2", "d3", "b3", "c4", "1c2-", "1d3<", "1b3>", "1c4+", "Cc2", "a1", "1c2-", "a2",
    ]
        .iter()
    {
        let mv = board.move_from_san(move_string).unwrap();
        board.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        board.do_move(mv);
        moves.clear();
    }
    board.generate_moves(&mut moves);
    assert!(
        moves.contains(&board.move_from_san("5c3>").unwrap()),
        "5c3> was not a legal move among {:?} on board\n{:?}",
        moves,
        board
    );

    assert!(
        !moves.contains(&board.move_from_san("6c3>").unwrap()),
        "6c3> was a legal move among {:?} on board\n{:?}",
        moves,
        board
    );
}

#[test]
pub fn start_pos_perf_test() {
    let mut board = Board::default();
    perft_check_answers(&mut board, &[1, 75, 5400, 348_080, 21_536_636]);
}

pub fn perft(board: &mut Board, depth: u16) -> u64 {
    if depth == 0 {
        1
    } else {
        let mut moves = vec![];
        board.generate_moves(&mut moves);
        moves
            .into_iter()
            .map(|mv| {
                let reverse_move = board.do_move(mv);
                let num_moves = perft(board, depth - 1);
                board.reverse_move(reverse_move);
                num_moves
            })
            .sum()
    }
}

#[cfg(test)]
/// Verifies the perft result of a position against a known answer
pub fn perft_check_answers(board: &mut Board, answers: &[u64]) {
    for (depth, &answer) in answers.iter().enumerate() {
        assert_eq!(perft(board, depth as u16), answer);
    }
}