use crate::position::Position;
use crate::search;
use crate::search::MctsSetting;
use crate::tests::do_moves_and_check_validity;
use board_game_traits::Position as PositionTrait;
use pgn_traits::PgnPosition;
use std::time;
use std::time::Duration;

#[test]
fn play_on_low_time() {
    let time = Duration::from_millis(5);
    let position = <Position<5>>::default();
    search::play_move_time(position, time, MctsSetting::default());
}

#[test]
fn win_in_two_moves_test() {
    let move_strings = ["e5", "c3", "c2", "d5", "c1", "c5", "d3", "a4", "e3"];

    plays_correct_move_property(&move_strings, &["b4", "b5", "Cb4", "Cb5"]);
}

#[test]
fn black_win_in_one_move_test() {
    let move_strings = [
        "b4", "c2", "d2", "c4", "b2", "c3", "d3", "b3", "c2+", "b3>", "d3<", "c4-", "d4", "4c3<22",
        "c2", "c4", "d4<", "b4>", "d3", "b4", "b1", "d4", "b2+", "2a3>", "e1", "5b3-23", "b3",
        "d1", "e1<", "a5", "e1", "b5", "b3+", "2c4<", "e1+",
    ];

    plays_correct_move_property(&move_strings, &["3b4-", "b3", "Cb3", "e4", "Ce4", "c3<"]);
}

#[test]
fn white_can_win_in_one_move_test() {
    let move_strings = ["b4", "c2", "d2", "c4", "b2", "d4", "e2", "c3"];

    plays_correct_move_property(&move_strings, &["a2", "Ca2"]);
}

#[test]
fn black_avoid_loss_in_one_test() {
    let move_strings = ["b4", "c2", "d2", "c4", "b2", "d4", "e2"];

    plays_correct_move_property(&move_strings, &["a2", "Ca2", "Sa2"]);
}

#[test]
fn black_avoid_loss_in_one_test2() {
    let move_strings = [
        "b4", "c2", "d2", "d4", "b2", "c4", "e2", "a2", "c3", "b3", "b2+", "c4-", "c2+", "b2",
        "b1", "d1", "c2", "a3", "2b3-", "a2>", "b1+",
    ];
    plays_correct_move_property(&move_strings, &["d1+"]);
}

#[test]
fn black_avoid_less_in_one_test5() {
    let move_strings = [
        "b3", "c2", "d2", "c3", "b2", "d4", "b2+", "d3", "d2+", "c3>", "Cc3", "b4", "c3>", "d2",
        "2d3-", "b2", "c2<", "b4-", "2b2+", "c2", "3d2<", "d1", "b2", "c4", "2d3+", "c4>", "e1",
        "c4", "b4", "3d4<12", "d2", "d1+", "4c2>", "3b4-", "b2+", "d4-", "3b3-12",
    ];

    plays_correct_move_property(&move_strings, &["Sb4", "Cb4", "Sb5", "Cb5", "c4<", "2c4<"]);
}

#[test]
fn white_avoid_loss_in_one_test() {
    let move_strings = [
        "c4", "c3", "b4", "c4-", "d2", "b5", "b3", "b5-", "b3>", "d4", "2c3-", "c4", "d3", "d4-",
        "d4", "c4-", "b2", "c4", "d4-", "2c3>", "d2+", "Sb3", "5d3-23", "b3-", "d4", "2b2>11",
        "3c2+21", "b3", "b2", "b3-", "c2", "b3", "c5", "2b2>", "b2", "b3-", "b3", "2b4-", "d5",
        "b4", "2c4<", "3b3+", "2c3-", "2b2>", "3d1<", "3c2-", "d1", "5b4-14",
    ];

    plays_correct_move_property(&move_strings, &["Cb5", "Sb5", "b5", "c5<", "d1<"]);
}

#[test]
fn white_avoid_loss_in_one_test2() {
    let move_strings = [
        "c4", "c3", "b3", "b4", "d3", "b4-", "b2", "b4", "d4", "c4-", "d2", "c4", "c2", "2b3-",
        "c2+", "c4-", "d3<", "d5", "4c3>22", "d5-", "2d3+", "Sc4", "3d4-", "c4-", "c4", "2c3>",
        "c4<", "5d3+", "2b4-11", "b3-", "Se1", "4d4<13", "c3", "5b2>32", "c3-", "3d2<", "b3",
        "3b4-", "b4", "c4<", "d3", "c4", "d3+", "c4>", "e4", "2d4>", "2e3+", "2d4>", "d3", "a4",
    ];

    plays_correct_move_property(&move_strings, &["Cd4", "Sd4", "Cc4", "Sc4"]);
}

#[test]
fn do_not_play_suicide_move_as_black_test() {
    let move_strings = [
        "c4", "c2", "d2", "c3", "b2", "d3", "d2+", "b3", "d2", "b4", "c2+", "b3>", "2d3<", "c4-",
        "d4", "5c3<23", "c2", "c4", "d4<", "d3", "d2+", "c3+", "Cc3", "2c4>", "c3<", "d2", "c3",
        "d2+", "c3+", "b4>", "2b3>11", "3c4-12", "d2", "c4", "b4", "c5", "b3>", "c4<", "3c3-",
        "e5", "e2",
    ];

    let mut position = <Position<5>>::default();
    do_moves_and_check_validity(&mut position, &move_strings);

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    assert!(moves.contains(&position.move_from_san("2a3-11").unwrap()));
    assert!(search::mcts(position.clone(), 10_000).0 != position.move_from_san("2a3-11").unwrap());
}

#[test]
fn do_not_play_suicide_move_as_black_test2() {
    let move_strings = [
        "d3", "c2", "d2", "c3", "d2+", "c4", "d2", "b3", "c2+", "b3>", "2d3<", "c4-", "b2",
        "5c3+23", "c2", "d3", "d2+", "b4", "d2", "d4", "2d3+", "b3", "b2+", "b4-", "d3", "b2",
        "b1", "b2>", "d2<", "c3-", "c1", "3b3>12", "c1+", "c3-", "3d4-", "4c2<22", "b1+", "2a2>",
        "d2", "3b2<", "d2<", "2b2>", "d4", "d2", "5d3-14",
    ];

    let mut position = <Position<5>>::default();
    do_moves_and_check_validity(&mut position, &move_strings);

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    assert!(moves.contains(&position.move_from_san("2c5>11").unwrap()));
    assert!(search::mcts(position.clone(), 10_000).0 != position.move_from_san("2c5>11").unwrap());
}

#[test]
fn do_not_instamove_into_loss() {
    let mut position = <Position<5>>::start_position();
    let move_strings = [
        "e1", "a5", "Cc3", "d1", "c1", "b1", "c2", "1b1>1", "b1", "2c1<2", "c1", "Ca1", "c4",
        "3b1>3", "1c2-1", "1d1<1", "Sd1", "1a1>1", "b3", "a1", "1d1<1", "1b1>1", "d3", "2c1>2",
        "Sb1", "1d1<1", "d2", "1e1<1", "c2", "e1", "1d2-1", "1e1<1",
    ];

    do_moves_and_check_validity(&mut position, &move_strings);

    let (best_move, _) = search::play_move_time(
        position.clone(),
        time::Duration::from_secs(1),
        MctsSetting::default(),
    );

    for move_string in ["b2", "c5", "b4", "d4"].iter() {
        assert_ne!(best_move, position.move_from_san(move_string).unwrap());
    }
}

#[test]
fn do_not_play_suicide_move_as_black_test3() {
    let move_strings = [
        "c3", "c2", "d2", "b4", "c2+", "d4", "c2", "b2", "c2<", "d5", "c2", "a3", "e2", "a2", "b1",
        "a1", "d3", "c4", "2c3+", "d4<", "Cc3", "3c4>12", "c3+", "e1", "c3", "a1>", "d1", "c5",
        "b5", "b4+", "d3+",
    ];

    let mut position = <Position<5>>::default();
    do_moves_and_check_validity(&mut position, &move_strings);

    let mut moves = vec![];
    position.generate_moves(&mut moves);
    assert!(moves.contains(&position.move_from_san("2b5>11").unwrap()));
    assert!(search::mcts(position.clone(), 10_000).0 != position.move_from_san("2b5>11").unwrap());
}

fn plays_correct_move_property(move_strings: &[&str], correct_moves: &[&str]) {
    let mut position = <Position<5>>::default();
    let mut moves = vec![];

    do_moves_and_check_validity(&mut position, move_strings);

    position.generate_moves(&mut moves);

    let nodes = 25000;
    let settings = search::MctsSetting::default().arena_size_for_nodes(nodes);
    let mut mcts = search::MonteCarloTree::with_settings(position.clone(), settings);

    for move_string in correct_moves {
        assert_eq!(
            *move_string,
            position.move_to_san(&position.move_from_san(move_string).unwrap())
        );
        assert!(
            moves.contains(&position.move_from_san(move_string).unwrap()),
            "Candidate move {} was not among legal moves {:?} on board\n{:?}",
            move_string,
            moves,
            position
        );
    }

    for i in 1..=nodes {
        mcts.select().unwrap();
        if i % 5000 == 0 {
            let (best_move, _score) = mcts.best_move();
            assert!(correct_moves
                                .iter()
                                .any(|mv| best_move == position.move_from_san(mv).unwrap()),
                    "{} didn't play one of the correct moves {:?}, {} played instead after {} iterations on board:\n{:?}",
                    position.side_to_move(), correct_moves, position.move_to_san(&best_move), i, position);
        }
    }
}

#[test]
// This test is probabilistic, and can theoretically fail due to random chance
fn best_move_temperature_test() {
    let position = <Position<5>>::start_position();
    let moves = vec![
        (position.move_from_san("a1").unwrap(), 0.5),
        (position.move_from_san("a2").unwrap(), 0.1),
        (position.move_from_san("a3").unwrap(), 0.1),
        (position.move_from_san("a4").unwrap(), 0.1),
        (position.move_from_san("a5").unwrap(), 0.1),
        (position.move_from_san("b1").unwrap(), 0.1),
    ];

    let mut rng = rand::thread_rng();
    let mut top_move_selected = 0;
    let mut b1_selected = 0;

    for _ in 0..1000 {
        let move_string_selected = search::best_move(&mut rng, 1.0, &moves).to_string::<5>();
        if move_string_selected == "a1" {
            top_move_selected += 1;
        } else if move_string_selected == "b1" {
            b1_selected += 1;
        }
    }

    println!("top move {}, a2 {}", top_move_selected, b1_selected);
    assert!(top_move_selected > 400);
    assert!(top_move_selected < 600);
    assert!(b1_selected > 75);
    assert!(b1_selected < 150);
}
