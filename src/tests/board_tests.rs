use board_game_traits::{Color, Position as PositionTrait};
use board_game_traits::{GameResult, GameResult::*};
use pgn_traits::PgnPosition;

use crate::position::Direction::*;
use crate::position::Piece::{BlackCap, BlackFlat, WhiteFlat, WhiteWall};
use crate::position::Position;
use crate::position::{squares_iterator, Piece, Role, Square, Stack};
use crate::position::{ExpMove, Move};
use crate::tests::do_moves_and_check_validity;
use crate::{position as board_mod, search};

#[test]
fn default_board_test() {
    let position = <Position<5>>::default();
    for square in squares_iterator::<5>() {
        assert!(position.get_stack(square).is_empty());
    }
    let position = <Position<6>>::default();
    for square in squares_iterator::<6>() {
        assert!(position.get_stack(square).is_empty());
    }
}

#[test]
fn get_set_test() {
    let pieces = [WhiteFlat, BlackFlat, BlackFlat, WhiteWall];
    let mut position = <Position<5>>::default();

    let mut stack = position.get_stack(Square::from_u8(12));
    for &piece in pieces.iter() {
        stack.push(piece);
    }
    position.set_stack(Square::from_u8(12), stack);

    assert_eq!(position.get_stack(Square::from_u8(12)).len(), 4);
    assert_eq!(
        position.get_stack(Square::from_u8(12)).top_stone(),
        Some(WhiteWall)
    );

    for (i, &piece) in pieces.iter().enumerate() {
        assert_eq!(
            Some(piece),
            position.get_stack(Square::from_u8(12)).get(i as u8),
            "{:?}",
            position.get_stack(Square::from_u8(12))
        );
    }

    let mut stack = position.get_stack(Square::from_u8(12));
    for &piece in pieces.iter().rev() {
        assert_eq!(Some(piece), stack.pop(), "{:?}", position);
    }
    position.set_stack(Square::from_u8(12), stack);

    assert!(position.get_stack(Square::from_u8(12)).is_empty());

    let mut stack = position.get_stack(Square::from_u8(12));
    for &piece in pieces.iter() {
        stack.push(piece);
    }
    position.set_stack(Square::from_u8(12), stack);

    let mut stack = position.get_stack(Square::from_u8(12));
    for &piece in pieces.iter() {
        assert_eq!(piece, stack.remove(0), "{:?}", stack);
    }
}

#[test]
fn flatten_stack_test() {
    let mut stack = Stack::default();
    stack.push(WhiteWall);
    stack.push(BlackCap);
    assert_eq!(stack.get(0), Some(WhiteFlat));
    assert_eq!(stack.pop(), Some(BlackCap));
    assert_eq!(stack.pop(), Some(WhiteFlat));
}

#[test]
fn correct_number_of_directions_5s_test() {
    assert_eq!(
        squares_iterator::<5>()
            .flat_map(|square| square.directions())
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn correct_number_of_neighbours_test() {
    assert_eq!(
        squares_iterator::<5>()
            .flat_map(|square| square.neighbors())
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn correct_number_of_legal_directions_test() {
    assert_eq!(
        squares_iterator::<5>()
            .flat_map(|square| [North, South, East, West]
                .iter()
                .filter_map(move |&direction| square.go_direction(direction)))
            .count(),
        4 * 2 + 12 * 3 + 9 * 4
    );
}

#[test]
fn stones_left_behind_by_stack_movement_test() {
    let mut position: Position<5> = <Position<5>>::default();

    do_moves_and_check_validity(&mut position, &["d3", "c3", "c4", "1d3<", "1c4-", "Sc4"]);

    let mv = position.move_from_san("2c3<11").unwrap();
    if let ExpMove::Move(square, _direction, stack_movement) = mv.expand() {
        assert_eq!(
            position
                .top_stones_left_behind_by_move(square, &stack_movement)
                .collect::<Vec<_>>(),
            vec![
                Some(Piece::WhiteFlat),
                Some(Piece::BlackFlat),
                Some(Piece::WhiteFlat)
            ]
        );
    } else {
        panic!()
    }

    let mv = position.move_from_san("3c3<21").unwrap();
    if let ExpMove::Move(square, _direction, stack_movement) = mv.expand() {
        assert_eq!(
            position
                .top_stones_left_behind_by_move(square, &stack_movement)
                .collect::<Vec<_>>(),
            vec![None, Some(Piece::BlackFlat), Some(Piece::WhiteFlat)]
        );
    } else {
        panic!()
    }
}

#[test]
fn black_can_win_with_road_test() {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    for mv_san in [
        "e5", "c3", "c2", "d5", "c1", "c5", "d3", "a4", "e3", "b5", "b1", "a5",
    ]
    .iter()
    {
        let mv = position.move_from_san(mv_san).unwrap();
        position.generate_moves(&mut moves);
        assert!(moves.contains(&mv));
        position.do_move(mv);
        moves.clear();
    }
    assert_eq!(position.game_result(), Some(GameResult::BlackWin));
}

#[test]
fn big_8s_spread_test() {
    let mut position = <Position<8>>::default();
    do_moves_and_check_validity(
        &mut position,
        &[
            "a1", "b1", "c1", "a2", "d1", "a3", "e1", "a4", "e1<", "a4-", "2d1<", "2a3-", "3c1<",
            "3a2-", "4b1<", "b1",
        ],
    );
    let mut moves = vec![];
    position.generate_moves(&mut moves);
    assert_eq!(moves.len(), 254 + 254 + (62 * 3));
    for mv in moves.iter() {
        assert_eq!(*mv, Move::compress(mv.expand()));

        if let ExpMove::Move(_, _, stack_movement) = mv.expand() {
            assert_ne!(stack_movement.into_inner(), 0)
        }

        let old_position = position.clone();
        let reverse_move = position.do_move(*mv);
        position.reverse_move(reverse_move);
        assert_eq!(position, old_position);
    }
}

#[test]
fn game_win_test() {
    let mut position = <Position<5>>::default();
    for mv in [
        Move::placement(Role::Flat, Square::from_u8(13)),
        Move::placement(Role::Flat, Square::from_u8(12)),
        Move::placement(Role::Flat, Square::from_u8(7)),
        Move::placement(Role::Flat, Square::from_u8(14)),
        Move::placement(Role::Flat, Square::from_u8(2)),
        Move::placement(Role::Flat, Square::from_u8(11)),
        Move::placement(Role::Flat, Square::from_u8(17)),
        Move::placement(Role::Flat, Square::from_u8(10)),
    ]
    .iter()
    {
        position.do_move(*mv);
        assert!(position.game_result().is_none());
    }
    position.do_move(Move::placement(Role::Flat, Square::from_u8(22)));
    assert_eq!(position.game_result(), Some(GameResult::WhiteWin));
}

#[test]
fn game_win_test2() {
    let mut position = <Position<5>>::default();
    for mv in [
        Move::placement(Role::Flat, Square::from_u8(7)),
        Move::placement(Role::Flat, Square::from_u8(12)),
        Move::placement(Role::Flat, Square::from_u8(14)),
        Move::placement(Role::Flat, Square::from_u8(2)),
        Move::placement(Role::Flat, Square::from_u8(13)),
        Move::placement(Role::Flat, Square::from_u8(17)),
        Move::placement(Role::Flat, Square::from_u8(11)),
        Move::placement(Role::Flat, Square::from_u8(22)),
    ]
    .iter()
    {
        position.do_move(*mv);
        assert!(position.game_result().is_none());
    }
    position.do_move(Move::placement(Role::Flat, Square::from_u8(10)));
    assert_eq!(position.game_result(), Some(GameResult::WhiteWin));
}

#[test]
fn double_road_wins_test() {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    let move_strings = [
        "a4", "a5", "b5", "b4", "c5", "c4", "1c5-", "d4", "d5", "e4", "e5", "c3",
    ];
    do_moves_and_check_validity(&mut position, &move_strings);

    position.generate_moves(&mut moves);
    assert!(moves.contains(&position.move_from_san("1c4+").unwrap()));
    moves.clear();

    let reverse_move = position.do_move(position.move_from_san("1c4+").unwrap());
    assert_eq!(position.game_result(), Some(WhiteWin));
    position.reverse_move(reverse_move);

    position = position.flip_board_y();

    position.generate_moves(&mut moves);
    assert!(moves.contains(&position.move_from_san("1c2-").unwrap()));

    position.do_move(position.move_from_san("1c2-").unwrap());
    assert_eq!(position.game_result(), Some(WhiteWin));
}

// Black is behind by one point, with one stone left to place
// Check that placing it as a wall is suicide, but placing it flat is not
#[test]
fn suicide_into_points_loss_test() {
    let mut position = <Position<5>>::start_position();
    let move_strings = [
        "a1", "e5", "e3", "Cc3", "e4", "e2", "d3", "c3>", "d4", "b2", "c3", "c2", "c4", "d2",
        "c3-", "a2", "c3", "c1", "2c2<", "c2", "c3-", "b1", "e3-", "e1", "2e2-", "d1", "2c2-",
        "a2>", "a4", "4b2+112", "a4>", "2b5-", "c4<", "b3+", "e2", "5b4>122", "3e1<", "e1", "e5-",
        "3d4>", "e2-", "b3", "c3", "c2", "b2", "a3", "c5", "c2+", "c4-", "2d3<", "Cc2", "b3-",
        "a2", "e2", "a2>", "b1>", "c2-", "e2-", "5c1>32", "Se2", "a2", "a1+", "2b2<", "c2", "c1",
        "b1", "b2-", "c2-", "4a2+13", "c2", "5e1<23",
    ];
    do_moves_and_check_validity(&mut position, &move_strings);

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    for mv in moves.iter() {
        let reverse_move = position.do_move(*mv);
        match mv.expand() {
            ExpMove::Place(Role::Wall, _) => assert_eq!(
                position.game_result(),
                Some(GameResult::WhiteWin),
                "Placing a wall is suicide"
            ),
            ExpMove::Place(Role::Flat, _) => assert_eq!(
                position.game_result(),
                Some(GameResult::Draw),
                "Placing a flatstone is a draw"
            ),
            _ => (),
        }
        position.reverse_move(reverse_move);
    }

    assert!(moves
        .iter()
        .any(|mv| matches!(mv.expand(), ExpMove::Place(Role::Wall, _))));
    assert!(moves
        .iter()
        .any(|mv| matches!(mv.expand(), ExpMove::Place(Role::Flat, _))));
}

#[test]
fn suicide_into_road_loss_test() {
    let move_strings = [
        "a5", "a1", "Cc3", "Cd3", "c2", "c4", "b3", "c1", "b2", "d2", "b4", "e3", "b5", "a5>",
        "b1", "c5", "a5", "2b5-", "a4", "Sa3", "b5", "a3+", "d4", "d1", "c3+", "3b4-", "e4", "d3+",
        "c3", "4b3>", "b3", "5c3<", "c3", "c1<", "a1>", "d2<", "3b1+12", "d3", "5b3>113", "2d4-",
        "b4", "2a4>", "2b2>11", "d3>", "2c4-", "c4", "d4", "4e3+13", "a3", "3b4-", "3c3+12",
    ];

    let mut position = <Position<5>>::start_position();

    do_moves_and_check_validity(&mut position, &move_strings);

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    let mv = position.move_from_lan("e4-").unwrap();

    assert!(moves.contains(&mv));
    position.do_move(mv);
    assert_eq!(position.game_result(), Some(WhiteWin))
}

#[test]
fn games_ends_when_board_is_full_test() {
    let mut position = <Position<5>>::start_position();
    let move_strings: Vec<String> = squares_iterator::<5>()
        .skip(1)
        .map(|sq| sq.to_string())
        .collect();
    do_moves_and_check_validity(
        &mut position,
        &(move_strings.iter().map(AsRef::as_ref).collect::<Vec<_>>()),
    );
    assert!(position.game_result().is_none());
    position.do_move(position.move_from_san("a5").unwrap());
    assert_eq!(
        position.game_result(),
        Some(WhiteWin),
        "Board is full, game should have ended:\n{:?}",
        position
    );
}

#[test]
fn every_move_is_suicide_test() {
    let mut position = <Position<5>>::start_position();

    do_moves_and_check_validity(&mut position, &["b3", "c4", "c4-", "b3>"]);

    for _ in 0..20 {
        do_moves_and_check_validity(&mut position, &["c4", "b3", "c4-", "b3>"]);
    }
    let mut moves = vec![];
    position.generate_moves(&mut moves);
    for mv in moves {
        let reverse_move = position.do_move(mv);
        assert_eq!(position.game_result(), Some(BlackWin));
        position.reverse_move(reverse_move);
    }
}

#[test]
fn critical_square_test() {
    let move_strings = ["a1", "e5", "e4", "a2", "e3", "a3", "e2", "a4"];

    let mut position = <Position<5>>::default();

    do_moves_and_check_validity(&mut position, &move_strings);

    let e1 = Square::parse_square("e1").unwrap();
    let a5 = Square::parse_square("a5").unwrap();
    let group_data = position.group_data();
    assert!(group_data.is_critical_square(e1, Color::White));
    assert!(!group_data.is_critical_square(e1, Color::Black));

    assert!(group_data.is_critical_square(a5, Color::Black));
    assert!(!group_data.is_critical_square(a5, Color::White));

    assert_eq!(
        group_data
            .critical_squares(Color::White)
            .collect::<Vec<_>>(),
        vec![e1]
    );
    assert_eq!(
        group_data
            .critical_squares(Color::Black)
            .collect::<Vec<_>>(),
        vec![a5]
    );
}

#[test]
fn move_iterator_test() {
    let mut position = <Position<5>>::start_position();
    do_moves_and_check_validity(&mut position, &["a1", "e5"]);
    let mv = position.move_from_san("e5-").unwrap();
    match mv.expand() {
        ExpMove::Move(square, direction, stack_movement) => assert_eq!(
            board_mod::MoveIterator::<5>::new(square, direction, stack_movement)
                .map(|sq| sq.to_string())
                .collect::<Vec<String>>(),
            vec!["e5", "e4"]
        ),
        _ => panic!(),
    }
}

#[test]
fn repetitions_are_draws_test() {
    let mut position = <Position<5>>::start_position();
    do_moves_and_check_validity(&mut position, &["a1", "e5"]);

    let cycle_move_strings = ["e5-", "a1+", "e4+", "a2-"];
    do_moves_and_check_validity(&mut position, &cycle_move_strings);
    assert_eq!(position.game_result(), None);

    do_moves_and_check_validity(&mut position, &cycle_move_strings);
    assert_eq!(position.game_result(), Some(GameResult::Draw));

    do_moves_and_check_validity(&mut position, &["e4"]);
    assert_eq!(position.game_result(), None);
}

#[test]
fn fake_repetitions_are_not_draws_test() {
    let mut position = <Position<6>>::start_position();
    let move_strings = [
        "a1", "f1", "d3", "d4", "c3", "c4", "e3", "b4", "e4", "b3", "e5", "Ce2", "Cc5", "b5", "a4",
        "b2", "c5-", "a2", "2c4<", "c4", "a3", "c2", "d2", "c5", "d1", "e2<", "e2", "b6", "a3>",
        "2d2+", "3b4>", "f3", "f2", "f3<", "f3", "d5", "4c4>", "2d3<11", "Sb4", "3b3>12", "Sc4",
        "d2", "f4", "e6", "f5", "f6", "d6", "2e3>", "e3", "3d3>", "f4-", "e6-", "c4-", "4e3<31",
        "Sc4", "5c3>32", "d4-", "4d4>31", "6d3+15", "2e3<", "3f3+12", "e4>", "2f5-", "e4>",
    ];
    do_moves_and_check_validity(&mut position, &move_strings);
    assert_eq!(position.game_result(), None);
}

#[test]
fn parse_tps_test() {
    let tps_string = "x4,1/x5/x5/x5/2,x4 1 2";

    let mut position = <Position<5>>::start_position();
    do_moves_and_check_validity(&mut position, &["a1", "e5"]);
    assert_eq!(<Position<5>>::from_fen(tps_string).unwrap(), position);

    do_moves_and_check_validity(&mut position, &["c5"]);
    let tps_string = "x2,1,x,1/x5/x5/x5/2,x4 2 2";
    assert_eq!(<Position<5>>::from_fen(tps_string).unwrap(), position);
}

#[test]
fn join_too_many_groups_test() {
    let tps = "1,x,1,x,1,x/21121111S,x4,1/1,x,1,x,1,x/x6/x,2,2,2,2,x/2,x5 1 18";
    let position = <Position<6>>::from_fen(tps).unwrap();
    search::mcts(position, 1000);
}
