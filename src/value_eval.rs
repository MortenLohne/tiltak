use crate::bitboard::BitBoard;
use crate::board::{
    squares_iterator, BlackTr, Board, ColorTr, GroupData, Piece::*, Role::*, Square, WhiteTr,
    BOARD_AREA, BOARD_SIZE, NUM_SQUARE_SYMMETRIES, SQUARE_SYMMETRIES,
};
use board_game_traits::board::{Board as EvalBoard, Color};

pub(crate) fn static_eval_game_phase(
    board: &Board,
    group_data: &GroupData,
    coefficients: &mut [f32],
) {
    const FLAT_PSQT: usize = 0;
    const WALL_PSQT: usize = FLAT_PSQT + NUM_SQUARE_SYMMETRIES;
    const CAP_PSQT: usize = WALL_PSQT + NUM_SQUARE_SYMMETRIES;
    const OUR_STACK_PSQT: usize = CAP_PSQT + NUM_SQUARE_SYMMETRIES;
    const THEIR_STACK_PSQT: usize = OUR_STACK_PSQT + NUM_SQUARE_SYMMETRIES;

    let mut white_flat_count = 0;
    let mut black_flat_count = 0;

    for square in squares_iterator() {
        let stack = &board[square];
        if let Some(piece) = board[square].top_stone() {
            let i = square.0 as usize;
            match piece {
                WhiteFlat => {
                    coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] += 1.0;
                    white_flat_count += 1;
                }
                BlackFlat => {
                    coefficients[FLAT_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0;
                    black_flat_count += 1;
                }
                WhiteWall => coefficients[WALL_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                BlackWall => coefficients[WALL_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
                WhiteCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] += 1.0,
                BlackCap => coefficients[CAP_PSQT + SQUARE_SYMMETRIES[i]] -= 1.0,
            }
            if stack.height > 1 {
                let controlling_player = piece.color();
                let color_factor = piece.color().multiplier() as f32;
                for piece in stack.clone().into_iter().take(stack.height as usize - 1) {
                    if piece.color() == controlling_player {
                        coefficients[OUR_STACK_PSQT + SQUARE_SYMMETRIES[i]] += color_factor;
                    } else {
                        coefficients[THEIR_STACK_PSQT + SQUARE_SYMMETRIES[i]] -= color_factor;
                    }
                }
            }
        }
    }

    const SIDE_TO_MOVE: usize = THEIR_STACK_PSQT + NUM_SQUARE_SYMMETRIES;
    const FLATSTONE_LEAD: usize = SIDE_TO_MOVE + 3;
    const NUMBER_OF_GROUPS: usize = FLATSTONE_LEAD + 3;

    // Give the side to move a bonus/malus depending on flatstone lead
    let white_flatstone_lead = white_flat_count - black_flat_count;

    // Bonus/malus depending on the number of groups each side has
    let mut seen_groups = [false; BOARD_AREA + 1];
    seen_groups[0] = true;

    let number_of_groups = squares_iterator()
        .map(|square| {
            let group_id = group_data.groups[square] as usize;
            if !seen_groups[group_id] {
                seen_groups[group_id] = true;
                board[square].top_stone().unwrap().color().multiplier()
            } else {
                0
            }
        })
        .sum::<isize>() as f32;

    let opening_scale_factor = f32::min(
        f32::max((24.0 - board.half_moves_played() as f32) / 12.0, 0.0),
        1.0,
    );
    let endgame_scale_factor = f32::min(
        f32::max((board.half_moves_played() as f32 - 24.0) / 24.0, 0.0),
        1.0,
    );
    let middlegame_scale_factor = 1.0 - opening_scale_factor - endgame_scale_factor;

    debug_assert!(middlegame_scale_factor <= 1.0);
    debug_assert!(opening_scale_factor == 0.0 || endgame_scale_factor == 0.0);

    coefficients[SIDE_TO_MOVE] = board.side_to_move().multiplier() as f32 * opening_scale_factor;
    coefficients[FLATSTONE_LEAD] = white_flatstone_lead as f32 * opening_scale_factor;
    coefficients[NUMBER_OF_GROUPS] = number_of_groups * opening_scale_factor;

    coefficients[SIDE_TO_MOVE + 1] =
        board.side_to_move().multiplier() as f32 * middlegame_scale_factor;
    coefficients[FLATSTONE_LEAD + 1] = white_flatstone_lead as f32 * middlegame_scale_factor;
    coefficients[NUMBER_OF_GROUPS + 1] = number_of_groups * middlegame_scale_factor;

    coefficients[SIDE_TO_MOVE + 2] =
        board.side_to_move().multiplier() as f32 * endgame_scale_factor;
    coefficients[FLATSTONE_LEAD + 2] = white_flatstone_lead as f32 * endgame_scale_factor;
    coefficients[NUMBER_OF_GROUPS + 2] = number_of_groups * endgame_scale_factor;

    const CRITICAL_SQUARES: usize = NUMBER_OF_GROUPS + 3;

    for critical_square in group_data.critical_squares(Color::White) {
        critical_squares_eval::<WhiteTr, BlackTr>(
            board,
            critical_square,
            coefficients,
            CRITICAL_SQUARES,
        );
    }

    for critical_square in group_data.critical_squares(Color::Black) {
        critical_squares_eval::<BlackTr, WhiteTr>(
            board,
            critical_square,
            coefficients,
            CRITICAL_SQUARES,
        );
    }

    const CAPSTONE_OVER_OWN_PIECE: usize = CRITICAL_SQUARES + 6;
    const CAPSTONE_ON_STACK: usize = CAPSTONE_OVER_OWN_PIECE + 1;
    const STANDING_STONE_ON_STACK: usize = CAPSTONE_ON_STACK + 1;
    const FLAT_STONE_NEXT_TO_OUR_STACK: usize = STANDING_STONE_ON_STACK + 1;
    const STANDING_STONE_NEXT_TO_OUR_STACK: usize = FLAT_STONE_NEXT_TO_OUR_STACK + 1;
    const CAPSTONE_NEXT_TO_OUR_STACK: usize = STANDING_STONE_NEXT_TO_OUR_STACK + 1;

    squares_iterator()
        .map(|sq| (sq, &board[sq]))
        .filter(|(_, stack)| stack.len() > 1)
        .for_each(|(square, stack)| {
            let top_stone = stack.top_stone().unwrap();
            let controlling_player = top_stone.color();
            let color_factor = top_stone.color().multiplier() as f32;

            // Extra bonus for having your capstone over your own piece
            if top_stone.role() == Cap
                && stack.get(stack.len() - 2).unwrap().color() == controlling_player
            {
                coefficients[CAPSTONE_OVER_OWN_PIECE] += color_factor;
            }

            match top_stone.role() {
                Cap => coefficients[CAPSTONE_ON_STACK] += color_factor,
                Flat => (),
                Wall => coefficients[STANDING_STONE_ON_STACK] += color_factor,
            }

            // Malus for them having stones next to our stack with flat stones on top
            for neighbour in square.neighbours() {
                if let Some(neighbour_top_stone) = board[neighbour].top_stone() {
                    if top_stone.role() == Flat && neighbour_top_stone.color() != controlling_player
                    {
                        match neighbour_top_stone.role() {
                            Flat => {
                                coefficients[FLAT_STONE_NEXT_TO_OUR_STACK] +=
                                    color_factor * stack.len() as f32
                            }
                            Wall => {
                                coefficients[STANDING_STONE_NEXT_TO_OUR_STACK] +=
                                    color_factor * stack.len() as f32
                            }
                            Cap => {
                                coefficients[CAPSTONE_NEXT_TO_OUR_STACK] +=
                                    color_factor * stack.len() as f32
                            }
                        }
                    }
                }
            }
        });

    // Number of pieces in each line
    const NUM_LINES_OCCUPIED: usize = CAPSTONE_NEXT_TO_OUR_STACK + 1;
    // Number of lines with at least one road stone
    const LINE_CONTROL: usize = NUM_LINES_OCCUPIED + BOARD_SIZE + 1;

    let mut num_ranks_occupied_white = 0;
    let mut num_files_occupied_white = 0;
    let mut num_ranks_occupied_black = 0;
    let mut num_files_occupied_black = 0;

    for line in BitBoard::all_lines().iter() {
        line_score::<WhiteTr, BlackTr>(&group_data, *line, coefficients, LINE_CONTROL);
        line_score::<BlackTr, WhiteTr>(&group_data, *line, coefficients, LINE_CONTROL);
    }

    for i in 0..BOARD_SIZE as u8 {
        if !WhiteTr::road_stones(&group_data).rank(i).is_empty() {
            num_ranks_occupied_white += 1;
        }
        if !BlackTr::road_stones(&group_data).rank(i).is_empty() {
            num_ranks_occupied_black += 1;
        }
    }

    for i in 0..BOARD_SIZE as u8 {
        if !WhiteTr::road_stones(&group_data).file(i).is_empty() {
            num_files_occupied_white += 1;
        }
        if !BlackTr::road_stones(&group_data).file(i).is_empty() {
            num_files_occupied_black += 1;
        }
    }

    coefficients[NUM_LINES_OCCUPIED + num_ranks_occupied_white] += 1.0;
    coefficients[NUM_LINES_OCCUPIED + num_files_occupied_white] += 1.0;
    coefficients[NUM_LINES_OCCUPIED + num_ranks_occupied_black] -= 1.0;
    coefficients[NUM_LINES_OCCUPIED + num_files_occupied_black] -= 1.0;

    const _NEXT_CONST: usize = LINE_CONTROL + 2 * (BOARD_SIZE + 1);

    assert_eq!(_NEXT_CONST, coefficients.len());
}

/// Give bonus for our critical squares
fn critical_squares_eval<Us: ColorTr, Them: ColorTr>(
    board: &Board,
    critical_square: Square,
    coefficients: &mut [f32],
    critical_squares: usize,
) {
    let top_stone = board[critical_square].top_stone;
    if top_stone.is_none() {
        coefficients[critical_squares] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Us::wall_piece()) {
        coefficients[critical_squares + 1] += Us::color().multiplier() as f32;
    } else if top_stone == Some(Them::flat_piece()) {
        coefficients[critical_squares + 2] += Us::color().multiplier() as f32;
    }
    // Their capstone or wall
    else {
        coefficients[critical_squares + 3] += Us::color().multiplier() as f32
    }

    // Bonus for having our cap next to our critical square
    for neighbour in critical_square.neighbours() {
        if board[neighbour].top_stone() == Some(Us::cap_piece()) {
            coefficients[critical_squares + 4] += Us::color().multiplier() as f32;
            // Further bonus for a capped stack next to our critical square
            for piece in board[neighbour].clone().into_iter() {
                if piece == Us::flat_piece() {
                    coefficients[critical_squares + 5] += Us::color().multiplier() as f32;
                }
            }
        }
    }
}

fn line_score<Us: ColorTr, Them: ColorTr>(
    group_data: &GroupData,
    line: BitBoard,
    coefficients: &mut [f32],
    line_control: usize,
) {
    let road_pieces_in_line = (Us::road_stones(group_data) & line).count();

    coefficients[line_control + road_pieces_in_line as usize] += Us::color().multiplier() as f32;

    let block_their_line = line_control + BOARD_SIZE + 1;

    // Bonus for blocking their lines
    if road_pieces_in_line >= 3 {
        coefficients[block_their_line + road_pieces_in_line as usize - 3] +=
            ((Them::flats(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
        coefficients[block_their_line + 2 + road_pieces_in_line as usize - 3] +=
            ((Them::walls(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
        coefficients[block_their_line + 4 + road_pieces_in_line as usize - 3] +=
            ((Them::caps(group_data) & line).count() as isize * Them::color().multiplier()) as f32;
    }
}
