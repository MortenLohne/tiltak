use dfdx::prelude::{Linear, ReLU, Tanh};

use crate::position::{num_line_symmetries, num_square_symmetries};

pub const NUM_VALUE_FEATURES_4S: usize = 87;
pub const NUM_POLICY_FEATURES_4S: usize = 157;

pub const NUM_VALUE_FEATURES_5S: usize = 124;
pub const NUM_POLICY_FEATURES_5S: usize = 176;

pub const NUM_VALUE_FEATURES_6S: usize = 134;
pub const NUM_POLICY_FEATURES_6S: usize = 186;

pub type ValueModel<const N: usize> = (
    (Linear<N, 32>, ReLU),
    (Linear<32, 32>, ReLU),
    (Linear<32, 1>, Tanh),
);

#[derive(Debug)]
pub struct ValueFeatures<'a> {
    pub flat_psqt: &'a mut [f32],
    pub wall_psqt: &'a mut [f32],
    pub cap_psqt: &'a mut [f32],
    pub supports_psqt: &'a mut [f32],
    pub captives_psqt: &'a mut [f32],
    pub shallow_supports_per_piece: &'a mut [f32],
    pub deep_supports_per_piece: &'a mut [f32],
    pub shallow_captives_per_piece: &'a mut [f32],
    pub deep_captives_per_piece: &'a mut [f32],
    pub side_to_move: &'a mut [f32],
    pub flatstone_lead: &'a mut [f32],
    pub i_number_of_groups: &'a mut [f32],
    pub critical_squares: &'a mut [f32],
    pub flat_next_to_our_stack: &'a mut [f32],
    pub wall_next_to_our_stack: &'a mut [f32],
    pub cap_next_to_our_stack: &'a mut [f32],
    pub num_lines_occupied: &'a mut [f32],
    pub line_control_empty: &'a mut [f32],
    pub line_control_their_blocking_piece: &'a mut [f32],
    pub line_control_other: &'a mut [f32],
    pub sidelined_cap: &'a mut [f32],
    pub fully_isolated_cap: &'a mut [f32],
    pub semi_isolated_cap: &'a mut [f32],
}

impl<'a> ValueFeatures<'a> {
    pub fn new<const S: usize>(coefficients: &'a mut [f32]) -> Self {
        assert_eq!(coefficients.len(), num_value_features::<S>());

        let (flat_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (wall_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (cap_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (supports_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (captives_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (shallow_supports_per_piece, coefficients) = coefficients.split_at_mut(4);
        let (deep_supports_per_piece, coefficients) = coefficients.split_at_mut(4);
        let (shallow_captives_per_piece, coefficients) = coefficients.split_at_mut(4);
        let (deep_captives_per_piece, coefficients) = coefficients.split_at_mut(4);
        let (side_to_move, coefficients) = coefficients.split_at_mut(3);
        let (flatstone_lead, coefficients) = coefficients.split_at_mut(3);
        let (i_number_of_groups, coefficients) = coefficients.split_at_mut(3);
        let (critical_squares, coefficients) = coefficients.split_at_mut(6);
        let (flat_next_to_our_stack, coefficients) = coefficients.split_at_mut(1);
        let (wall_next_to_our_stack, coefficients) = coefficients.split_at_mut(1);
        let (cap_next_to_our_stack, coefficients) = coefficients.split_at_mut(1);
        let (num_lines_occupied, coefficients) = coefficients.split_at_mut(S + 1);
        let (line_control_empty, coefficients) =
            coefficients.split_at_mut(S * num_line_symmetries::<S>());
        let (line_control_their_blocking_piece, coefficients) =
            coefficients.split_at_mut(S * num_line_symmetries::<S>());
        let (line_control_other, coefficients) =
            coefficients.split_at_mut(S * num_line_symmetries::<S>());
        let (sidelined_cap, coefficients) = coefficients.split_at_mut(3);
        let (fully_isolated_cap, coefficients) = coefficients.split_at_mut(3);
        let (semi_isolated_cap, coefficients) = coefficients.split_at_mut(3);

        assert!(coefficients.is_empty());

        ValueFeatures {
            flat_psqt,
            wall_psqt,
            cap_psqt,
            supports_psqt,
            captives_psqt,
            shallow_supports_per_piece,
            deep_supports_per_piece,
            shallow_captives_per_piece,
            deep_captives_per_piece,
            side_to_move,
            flatstone_lead,
            i_number_of_groups,
            critical_squares,
            flat_next_to_our_stack,
            wall_next_to_our_stack,
            cap_next_to_our_stack,
            num_lines_occupied,
            line_control_empty,
            line_control_their_blocking_piece,
            line_control_other,
            sidelined_cap,
            fully_isolated_cap,
            semi_isolated_cap,
        }
    }
}

#[derive(Debug)]
pub struct PolicyFeatures<'a> {
    pub decline_win: &'a mut [f32],
    pub place_to_win: &'a mut [f32],
    pub place_to_draw: &'a mut [f32],
    pub place_to_loss: &'a mut [f32],
    pub place_to_allow_opponent_to_end: &'a mut [f32],
    pub two_flats_left: &'a mut [f32],
    pub three_flats_left: &'a mut [f32],
    pub flat_psqt: &'a mut [f32],
    pub wall_psqt: &'a mut [f32],
    pub cap_psqt: &'a mut [f32],
    pub our_road_stones_in_line: &'a mut [f32],
    pub their_road_stones_in_line: &'a mut [f32],
    pub extend_single_group_base: &'a mut [f32],
    pub extend_single_group_linear: &'a mut [f32],
    pub extend_single_group_to_new_line_base: &'a mut [f32],
    pub extend_single_group_to_new_line_linear: &'a mut [f32],
    pub merge_two_groups_base: &'a mut [f32],
    pub merge_two_groups_linear: &'a mut [f32],
    pub block_merger_base: &'a mut [f32],
    pub block_merger_linear: &'a mut [f32],
    pub place_our_critical_square: &'a mut [f32],
    pub place_their_critical_square: &'a mut [f32],
    pub ignore_their_critical_square: &'a mut [f32],
    pub next_to_our_last_stone: &'a mut [f32],
    pub next_to_their_last_stone: &'a mut [f32],
    pub diagonal_to_our_last_stone: &'a mut [f32],
    pub diagonal_to_their_last_stone: &'a mut [f32],
    pub attack_strong_flats: &'a mut [f32],
    pub blocking_stone_blocks_extensions_of_two_flats: &'a mut [f32],
    pub attack_strong_stack_with_wall: &'a mut [f32],
    pub attack_strong_stack_with_cap: &'a mut [f32],
    pub attack_last_movement: &'a mut [f32],
    pub place_last_movement: &'a mut [f32],
    pub move_role_bonus: &'a mut [f32],
    pub simple_movement: &'a mut [f32],
    pub simple_capture: &'a mut [f32],
    pub simple_self_capture: &'a mut [f32],
    pub pure_spread: &'a mut [f32],
    pub fcd_highest_board: &'a mut [f32],
    pub fcd_highest_stack: &'a mut [f32],
    pub fcd_other: &'a mut [f32],
    pub stack_captured_by_movement: &'a mut [f32],
    pub stack_capture_in_strong_line: &'a mut [f32],
    pub stack_capture_in_strong_line_cap: &'a mut [f32],
    pub move_cap_onto_strong_line: &'a mut [f32],
    pub move_cap_onto_strong_line_with_critical_square: &'a mut [f32],
    pub recapture_stack_pure: &'a mut [f32],
    pub recapture_stack_impure: &'a mut [f32],
    pub move_last_placement: &'a mut [f32],
    pub continue_spread: &'a mut [f32],
    pub move_onto_critical_square: &'a mut [f32],
    pub spread_that_connects_groups_to_win: &'a mut [f32],
}

impl<'a> PolicyFeatures<'a> {
    #[inline(never)]
    pub fn new<const S: usize>(coefficients: &'a mut [f32]) -> PolicyFeatures<'a> {
        assert_eq!(coefficients.len(), num_policy_features::<S>());

        let (decline_win, coefficients) = coefficients.split_at_mut(1);
        let (place_to_win, coefficients) = coefficients.split_at_mut(1);
        let (place_to_draw, coefficients) = coefficients.split_at_mut(1);
        let (place_to_loss, coefficients) = coefficients.split_at_mut(1);
        let (place_to_allow_opponent_to_end, coefficients) = coefficients.split_at_mut(3);
        let (two_flats_left, coefficients) = coefficients.split_at_mut(2);
        let (three_flats_left, coefficients) = coefficients.split_at_mut(2);
        let (flat_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (wall_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (cap_psqt, coefficients) = coefficients.split_at_mut(num_square_symmetries::<S>());
        let (our_road_stones_in_line, coefficients) = coefficients.split_at_mut(S * 3);
        let (their_road_stones_in_line, coefficients) = coefficients.split_at_mut(S * 3);
        let (extend_single_group_to_new_line_base, coefficients) = coefficients.split_at_mut(3);
        let (extend_single_group_to_new_line_linear, coefficients) = coefficients.split_at_mut(3);
        let (extend_single_group_base, coefficients) = coefficients.split_at_mut(3);
        let (extend_single_group_linear, coefficients) = coefficients.split_at_mut(3);
        let (merge_two_groups_base, coefficients) = coefficients.split_at_mut(3);
        let (merge_two_groups_linear, coefficients) = coefficients.split_at_mut(3);
        let (block_merger_base, coefficients) = coefficients.split_at_mut(3);
        let (block_merger_linear, coefficients) = coefficients.split_at_mut(3);
        let (place_our_critical_square, coefficients) = coefficients.split_at_mut(1);
        let (place_their_critical_square, coefficients) = coefficients.split_at_mut(4);
        let (ignore_their_critical_square, coefficients) = coefficients.split_at_mut(2);
        let (next_to_our_last_stone, coefficients) = coefficients.split_at_mut(1);
        let (next_to_their_last_stone, coefficients) = coefficients.split_at_mut(1);
        let (diagonal_to_our_last_stone, coefficients) = coefficients.split_at_mut(1);
        let (diagonal_to_their_last_stone, coefficients) = coefficients.split_at_mut(1);
        let (attack_strong_flats, coefficients) = coefficients.split_at_mut(1);
        let (blocking_stone_blocks_extensions_of_two_flats, coefficients) =
            coefficients.split_at_mut(1);
        let (attack_strong_stack_with_wall, coefficients) = coefficients.split_at_mut(6);
        let (attack_strong_stack_with_cap, coefficients) = coefficients.split_at_mut(6);
        let (attack_last_movement, coefficients) = coefficients.split_at_mut(4);
        let (place_last_movement, coefficients) = coefficients.split_at_mut(3);
        let (move_role_bonus, coefficients) = coefficients.split_at_mut(3);
        let (simple_movement, coefficients) = coefficients.split_at_mut(3);
        let (simple_capture, coefficients) = coefficients.split_at_mut(4);
        let (simple_self_capture, coefficients) = coefficients.split_at_mut(4);
        let (pure_spread, coefficients) = coefficients.split_at_mut(2);
        let (fcd_highest_board, coefficients) = coefficients.split_at_mut(6);
        let (fcd_highest_stack, coefficients) = coefficients.split_at_mut(6);
        let (fcd_other, coefficients) = coefficients.split_at_mut(8);
        let (stack_captured_by_movement, coefficients) = coefficients.split_at_mut(1);
        let (stack_capture_in_strong_line, coefficients) = coefficients.split_at_mut(S - 3);
        let (stack_capture_in_strong_line_cap, coefficients) = coefficients.split_at_mut(S - 3);
        let (move_cap_onto_strong_line, coefficients) = coefficients.split_at_mut(S - 3);
        let (move_cap_onto_strong_line_with_critical_square, coefficients) =
            coefficients.split_at_mut(S - 3);
        let (recapture_stack_pure, coefficients) = coefficients.split_at_mut(3);
        let (recapture_stack_impure, coefficients) = coefficients.split_at_mut(3);
        let (move_last_placement, coefficients) = coefficients.split_at_mut(3);
        let (continue_spread, coefficients) = coefficients.split_at_mut(3);
        let (move_onto_critical_square, coefficients) = coefficients.split_at_mut(3);
        let (spread_that_connects_groups_to_win, coefficients) = coefficients.split_at_mut(1);

        assert!(coefficients.is_empty());

        PolicyFeatures {
            decline_win,
            place_to_win,
            place_to_draw,
            place_to_loss,
            place_to_allow_opponent_to_end,
            two_flats_left,
            three_flats_left,
            flat_psqt,
            wall_psqt,
            cap_psqt,
            our_road_stones_in_line,
            their_road_stones_in_line,
            extend_single_group_base,
            extend_single_group_linear,
            extend_single_group_to_new_line_base,
            extend_single_group_to_new_line_linear,
            merge_two_groups_base,
            merge_two_groups_linear,
            block_merger_base,
            block_merger_linear,
            place_our_critical_square,
            place_their_critical_square,
            ignore_their_critical_square,
            next_to_our_last_stone,
            next_to_their_last_stone,
            diagonal_to_our_last_stone,
            diagonal_to_their_last_stone,
            attack_strong_flats,
            blocking_stone_blocks_extensions_of_two_flats,
            attack_strong_stack_with_wall,
            attack_strong_stack_with_cap,
            attack_last_movement,
            place_last_movement,
            move_role_bonus,
            simple_movement,
            simple_capture,
            simple_self_capture,
            pure_spread,
            fcd_highest_board,
            fcd_highest_stack,
            fcd_other,
            stack_captured_by_movement,
            stack_capture_in_strong_line,
            stack_capture_in_strong_line_cap,
            move_cap_onto_strong_line,
            move_cap_onto_strong_line_with_critical_square,
            recapture_stack_pure,
            recapture_stack_impure,
            move_last_placement,
            continue_spread,
            move_onto_critical_square,
            spread_that_connects_groups_to_win,
        }
    }
}

pub fn num_value_features<const S: usize>() -> usize {
    match S {
        4 => NUM_VALUE_FEATURES_4S,
        5 => NUM_VALUE_FEATURES_5S,
        6 => NUM_VALUE_FEATURES_6S,
        _ => unimplemented!(),
    }
}

pub fn num_policy_features<const S: usize>() -> usize {
    match S {
        4 => NUM_POLICY_FEATURES_4S,
        5 => NUM_POLICY_FEATURES_5S,
        6 => NUM_POLICY_FEATURES_6S,
        _ => unimplemented!(),
    }
}

#[allow(clippy::unreadable_literal)]
pub const VALUE_PARAMS_4S: [f32; NUM_VALUE_FEATURES_4S] = [
    1.2960935,
    1.3755996,
    1.5638089,
    0.78314453,
    1.2711805,
    1.8741411,
    0.002932434,
    -0.0025715088,
    -0.00047107227,
    0.95423925,
    1.0412871,
    1.2421451,
    0.6032736,
    0.7569551,
    0.99624264,
    1.0294716,
    1.5590223,
    0.009879224,
    -0.0059808395,
    0.43383166,
    0.21582259,
    0.005999675,
    -0.0073812627,
    -0.6828443,
    -0.2672891,
    -0.008598236,
    0.0018530292,
    -0.7021679,
    -0.7136931,
    0.005024814,
    0.0038007405,
    1.8471439,
    1.7818584,
    2.2982776,
    1.2660044,
    0.92664075,
    2.0581422,
    -0.1795842,
    0.092309125,
    0.12011644,
    0.34540316,
    0.036157735,
    0.12752953,
    0.026238991,
    -0.0047925757,
    -0.005560641,
    0.019486373,
    -0.24060859,
    -0.0082427785,
    1.2213438,
    -1.4580884,
    -0.7276805,
    0.03937588,
    0.9138916,
    0.00059686694,
    -0.3603766,
    0.34891835,
    1.0730433,
    -0.0060374904,
    -0.23607875,
    0.35319594,
    0.7424935,
    -0.347289,
    -0.7963712,
    -0.41807485,
    0.107809,
    -0.5243122,
    -0.81458193,
    -0.2928778,
    0.33367318,
    0.04993621,
    -0.36220556,
    0.04083454,
    0.68550664,
    -0.07416395,
    -0.42777094,
    0.13790259,
    0.801516,
    0.0076200105,
    0.0061870757,
    -0.0077467104,
    -0.007937893,
    0.0076041985,
    -0.00032685045,
    -0.00023208652,
    0.008934194,
    -0.0025578667,
];

#[allow(clippy::unreadable_literal)]
pub const POLICY_PARAMS_4S: [f32; NUM_POLICY_FEATURES_4S] = [
    -3.5122416,
    1.415913,
    1.1052856,
    -2.5163739,
    -2.7743719,
    0.18032956,
    0.4586946,
    -0.23098475,
    0.1941311,
    -0.15256518,
    0.15466493,
    0.059777763,
    0.15994108,
    0.4249225,
    -0.28267896,
    -0.40255627,
    0.31419298,
    -0.0034274147,
    0.004325905,
    -0.0049592447,
    0.005938011,
    -0.2862876,
    0.45599967,
    1.1270785,
    -0.19198146,
    -0.077761486,
    -0.18037318,
    -0.25611642,
    0.008150065,
    0.0016175173,
    0.006870074,
    0.0040705632,
    0.15035017,
    -0.37990505,
    0.32469878,
    1.2303548,
    -0.5695383,
    -0.46843508,
    -0.42791986,
    0.75770706,
    0.005722007,
    0.0051122922,
    0.0010959748,
    0.0020771455,
    0.5804553,
    -0.20690332,
    0.004355209,
    -1.3740302,
    -0.09220675,
    0.008410923,
    0.4915791,
    0.44462475,
    0.0010137418,
    0.081576936,
    -0.33216107,
    -0.002478423,
    0.84405947,
    1.324717,
    0.009418681,
    0.1678178,
    -0.927851,
    0.008915393,
    0.21164335,
    0.41789484,
    0.008046815,
    -0.9325295,
    -0.35031798,
    -0.0008124877,
    2.4384596,
    0.22534506,
    0.97381663,
    0.0060925093,
    0.5779341,
    -5.1012006,
    -0.6555758,
    0.24332464,
    1.4057308,
    0.42615777,
    0.2845923,
    0.2861636,
    0.5823896,
    0.1815561,
    0.35023907,
    -0.34519213,
    0.5379025,
    -0.006355319,
    0.001523626,
    -0.00079395995,
    0.0012055086,
    -0.0018659113,
    0.004368524,
    0.0076928716,
    0.004360006,
    0.35640752,
    0.060082566,
    0.008424118,
    -0.0050589656,
    0.06644393,
    -0.20714737,
    -0.0010688612,
    -0.47961032,
    -0.31862563,
    -0.006010456,
    -0.8141309,
    -0.3157721,
    -0.005897157,
    0.12047051,
    0.14079803,
    -0.0075576873,
    -0.00044307671,
    -0.80057174,
    -0.39642796,
    0.009866308,
    0.0054509398,
    0.33524135,
    -1.1321205,
    0.009076214,
    1.2166464,
    1.634865,
    1.6140816,
    0.39080212,
    -0.026630996,
    -0.9424155,
    -0.8450094,
    -0.045945823,
    1.0065095,
    0.038155735,
    -0.008409168,
    -0.054947026,
    -0.24508871,
    -1.3722754,
    -0.99091464,
    -1.1681249,
    -0.15948702,
    -0.5293629,
    -0.3160614,
    0.4168786,
    -0.15563703,
    0.009882869,
    -0.0012246752,
    0.00716334,
    1.3710691,
    0.79837924,
    0.0011483384,
    -0.44209406,
    0.24983102,
    0.0071501285,
    0.27419892,
    0.8456319,
    -0.00934294,
    0.28680447,
    0.072626375,
    0.00928643,
    0.32709876,
    1.9311372,
    0.45529217,
    3.3508844,
];

#[allow(clippy::unreadable_literal)]
pub const VALUE_PARAMS_5S: [f32; NUM_VALUE_FEATURES_5S] = [
    0.6495996,
    0.6264087,
    0.6424795,
    0.63472235,
    0.59883386,
    0.5269288,
    0.5985967,
    0.9384296,
    1.0060751,
    1.364563,
    1.386277,
    1.3615981,
    0.7223717,
    1.1225014,
    1.1659535,
    1.9031403,
    2.0585349,
    2.1743684,
    0.7253143,
    0.8380635,
    0.88283247,
    0.8980131,
    0.9477068,
    1.0782348,
    0.39469373,
    0.502527,
    0.5224005,
    0.5923459,
    0.61175495,
    0.6634026,
    0.67725605,
    0.99516755,
    2.0030787,
    1.2296761,
    0.26849377,
    0.04693692,
    0.031255487,
    0.11079586,
    -0.5650867,
    -0.27574304,
    -0.3310668,
    -0.17738351,
    -0.52715564,
    -0.4431318,
    -0.50486153,
    -0.47285733,
    1.4774977,
    1.1283414,
    1.3637797,
    1.645141,
    0.79353493,
    1.2506193,
    -0.21600884,
    -0.14892322,
    -0.034205582,
    0.3117739,
    0.09561967,
    0.16677094,
    0.010250588,
    0.1002991,
    -0.0042300206,
    0.009783865,
    -0.17504315,
    -0.15301184,
    0.951331,
    -1.2315924,
    -0.71197677,
    -0.2197125,
    0.3175571,
    0.91201746,
    -0.0014058612,
    -0.4267075,
    -0.06294673,
    0.43246102,
    0.91643757,
    -0.00188174,
    -0.50255793,
    -0.006105375,
    0.5569835,
    0.90747476,
    -0.0077467104,
    -0.5143827,
    -0.06855094,
    0.55527574,
    0.9260895,
    -0.33957162,
    -0.6067062,
    -0.3362701,
    -0.06941078,
    0.24185306,
    -0.53865016,
    -0.7692165,
    -0.35123727,
    0.089146405,
    0.5574273,
    -0.61812353,
    -0.7705321,
    -0.3653969,
    0.11322055,
    0.5596796,
    -0.093101144,
    -0.34133473,
    -0.13495372,
    0.22224097,
    0.5908219,
    -0.43248397,
    -0.59067315,
    -0.17159313,
    0.3924391,
    0.86482835,
    -0.42668724,
    -0.5969889,
    -0.14788678,
    0.37836194,
    0.9474829,
    -0.29566485,
    -0.08247462,
    -0.18411462,
    -0.77349156,
    -0.2730902,
    -0.12613353,
    -0.42083722,
    0.08356222,
    -0.21141948,
];

#[allow(clippy::unreadable_literal)]
pub const POLICY_PARAMS_5S: [f32; NUM_POLICY_FEATURES_5S] = [
    -2.8982174,
    1.1347297,
    0.5880546,
    -2.155263,
    -2.0809464,
    0.047756672,
    0.46360356,
    -0.30339283,
    0.14567658,
    -0.13936101,
    0.086735584,
    -0.10499248,
    0.1719234,
    0.06882256,
    0.36661145,
    0.067540474,
    -0.10027594,
    -0.1980465,
    -0.29738614,
    -0.26358902,
    0.09364683,
    0.14889379,
    0.24165668,
    -1.2338128,
    -1.283696,
    -0.9389192,
    0.41474167,
    1.0061917,
    2.6652923,
    0.012733063,
    -0.27332026,
    -0.0026461477,
    0.4295934,
    0.7686701,
    -0.22018062,
    -0.16479777,
    -0.05287515,
    -0.085617825,
    -0.042914383,
    -0.011227002,
    0.007255429,
    0.47656575,
    1.0600402,
    -0.31365377,
    0.28511164,
    -0.17609422,
    0.023018852,
    0.37531763,
    0.40319285,
    -0.31701836,
    -0.44977722,
    -0.4277528,
    0.07002661,
    0.56100994,
    -0.51151276,
    -0.63861597,
    -0.2633256,
    0.4825807,
    2.1846237,
    0.59980726,
    -0.082617566,
    0.4634573,
    -0.018487548,
    -0.333237,
    -0.58962095,
    0.3689129,
    0.12881382,
    0.179892,
    0.25642273,
    0.017851207,
    -0.44762227,
    1.587862,
    0.3418366,
    0.82098764,
    -0.15256262,
    -0.18259698,
    -0.06891111,
    0.4090334,
    0.73204345,
    1.3715448,
    -0.22999127,
    -0.59859776,
    -0.34108704,
    2.1860538,
    0.21787868,
    2.1039927,
    2.5704584,
    0.5872586,
    -3.6105247,
    -0.7197803,
    0.39822865,
    1.406545,
    0.32680583,
    0.41463834,
    0.15160945,
    0.3139812,
    0.27085727,
    0.33552846,
    -0.04944666,
    0.3893713,
    -0.018882373,
    0.3977609,
    -0.13845246,
    -0.17858174,
    0.001478083,
    0.05475176,
    -0.27560374,
    -0.05942785,
    0.27636027,
    -0.055255897,
    -0.0013148449,
    0.008539425,
    0.338123,
    0.09161758,
    1.1256369,
    -0.44106278,
    -0.18141586,
    -0.2158977,
    -1.2049321,
    -0.30846742,
    -0.68806636,
    -0.041250374,
    0.025464198,
    0.56848913,
    0.7946283,
    -1.0177436,
    -0.323444,
    -0.1844594,
    0.15778622,
    0.2088733,
    -1.0382535,
    0.2620938,
    1.0634573,
    1.4434901,
    0.96456707,
    0.61136746,
    0.06877522,
    -0.8224534,
    -0.45572394,
    0.2980422,
    1.0080415,
    0.24921952,
    -0.006799847,
    -0.34533137,
    -0.61679673,
    -0.9565269,
    -0.8632079,
    -0.820081,
    -0.23231827,
    -0.7865291,
    -0.92631274,
    0.3733141,
    -0.015291728,
    -0.12742725,
    0.03405833,
    0.10353846,
    0.039584592,
    0.48560873,
    -0.24960202,
    -0.040668063,
    1.7836084,
    1.0814173,
    1.364929,
    -0.1881274,
    0.6383399,
    1.6785899,
    0.48927084,
    1.0496119,
    0.7293998,
    0.3091925,
    0.38659397,
    -0.025443463,
    0.35103112,
    2.5350509,
    0.9224742,
    3.1283102,
];

#[allow(clippy::unreadable_literal)]
pub const VALUE_PARAMS_6S: [f32; NUM_VALUE_FEATURES_6S] = [
    0.606187,
    0.5446117,
    0.54406977,
    0.59712934,
    0.6192196,
    0.5680083,
    0.22529893,
    0.61106944,
    0.6726761,
    0.9916199,
    1.1298188,
    1.2645868,
    0.039187424,
    0.5010584,
    0.65021455,
    1.2168105,
    1.5609205,
    1.8391675,
    0.51993597,
    0.62568444,
    0.68589634,
    0.70032203,
    0.77488726,
    0.75653815,
    0.36954045,
    0.44124892,
    0.44977257,
    0.54283947,
    0.57422334,
    0.5616728,
    0.6143715,
    0.85462576,
    1.4853287,
    1.0700635,
    0.2920155,
    -0.09491774,
    -0.098030955,
    -0.06769711,
    -0.55631524,
    -0.19200288,
    -0.17672306,
    -0.10198288,
    -0.401936,
    -0.4650831,
    -0.6883258,
    -0.3670923,
    1.0906948,
    0.86482877,
    1.0072992,
    1.4386058,
    0.88296574,
    1.1679748,
    -0.24030001,
    -0.19869824,
    -0.017924381,
    0.2546107,
    0.08701127,
    0.15877694,
    0.007067374,
    0.114150904,
    -0.02626372,
    0.022606345,
    -0.12556247,
    -0.12852018,
    0.93810076,
    -0.7260433,
    -0.52561533,
    -0.3214629,
    -0.0750549,
    0.2225322,
    0.50376153,
    -0.0038658213,
    -0.08316135,
    -0.017130688,
    0.12659669,
    0.36212754,
    0.5770378,
    -0.0026799915,
    -0.19310333,
    -0.068640575,
    0.13180655,
    0.3814123,
    0.46170714,
    -0.00032685045,
    -0.25205535,
    -0.104866266,
    0.07662872,
    0.3767714,
    0.5692452,
    -0.18182907,
    -0.44210964,
    -0.37306374,
    -0.2778329,
    -0.15621454,
    -0.023055106,
    -0.34905314,
    -0.46918887,
    -0.32652503,
    -0.17533821,
    -0.013424231,
    0.23628815,
    -0.34334445,
    -0.4333364,
    -0.2837256,
    -0.112828605,
    0.0774326,
    0.28103772,
    0.21040379,
    -0.06204036,
    -0.095324166,
    -0.011239963,
    0.1648674,
    0.2713752,
    -0.11068617,
    -0.24071796,
    -0.12877242,
    0.035447612,
    0.3048336,
    0.5276468,
    -0.26604328,
    -0.35593826,
    -0.1926967,
    0.025967682,
    0.33114037,
    0.61982065,
    -0.20731127,
    -0.10293561,
    -0.22141515,
    -0.54256046,
    -0.53815967,
    -0.1566519,
    -0.34156847,
    -0.004189716,
    -0.21910878,
];

#[allow(clippy::unreadable_literal)]
pub const POLICY_PARAMS_6S: [f32; NUM_POLICY_FEATURES_6S] = [
    -2.5490334,
    1.0907768,
    0.40533957,
    -2.1783361,
    -1.9656186,
    -0.11910041,
    0.3827465,
    -0.36281496,
    0.098670766,
    -0.061386336,
    0.07916245,
    -0.15045542,
    -0.05818011,
    -0.14055887,
    0.39565188,
    0.47307792,
    0.22034885,
    -0.29811528,
    -0.38566157,
    -0.32027248,
    0.17612194,
    0.31480277,
    0.37420526,
    -0.6160203,
    -0.99503464,
    -1.022247,
    -0.07620941,
    0.78106326,
    2.4153337,
    0.00958837,
    -0.24925435,
    -0.04037198,
    0.31065497,
    0.7048428,
    0.72551566,
    -0.19162245,
    -0.1526411,
    -0.025610907,
    0.015240486,
    0.02938831,
    -0.022778459,
    0.06385909,
    -0.23192704,
    0.25468063,
    0.49240685,
    0.45987254,
    -0.030208213,
    0.28996384,
    -0.059312735,
    0.035686687,
    0.271284,
    0.5214734,
    0.41869536,
    -0.37601113,
    -0.41671708,
    -0.39533865,
    -0.16467261,
    0.31418929,
    0.6781199,
    -1.0295482,
    -0.8719054,
    -0.48587862,
    0.40959165,
    1.2584724,
    1.6943147,
    0.54985595,
    -0.059545174,
    0.29058963,
    0.28040853,
    -0.2037632,
    0.053092442,
    0.39260617,
    0.09004204,
    0.22084866,
    0.1891087,
    0.21848248,
    -0.1416304,
    1.6414697,
    0.4064402,
    0.89881027,
    -0.106248684,
    -0.06722416,
    0.009552879,
    0.48911083,
    0.7436944,
    1.29594,
    -0.11115018,
    -0.24099883,
    -0.31982017,
    2.4572527,
    -0.09656046,
    2.1200073,
    1.8064114,
    0.53237724,
    -3.4268725,
    -0.6832289,
    0.72508657,
    1.391271,
    0.45646173,
    0.5518005,
    0.089685425,
    0.3283857,
    0.2916723,
    0.15119004,
    -0.014768326,
    0.3255551,
    -0.023082325,
    0.32838264,
    -0.3280263,
    -0.49792236,
    0.2521321,
    0.20311585,
    -0.19233644,
    -0.025560826,
    0.21606599,
    0.03086127,
    0.008243782,
    0.0015487336,
    0.2508647,
    0.07878296,
    1.8220797,
    -0.39295647,
    -0.14058624,
    -0.19435617,
    -1.3175937,
    -0.33471224,
    -0.50426483,
    -0.1613428,
    0.15429756,
    0.41458473,
    1.0414675,
    -1.0505555,
    -0.38385266,
    -0.119135685,
    0.10721053,
    0.3579779,
    -1.0944275,
    0.25238204,
    0.9679204,
    1.4556767,
    1.1631404,
    1.005055,
    -0.05970869,
    -0.86475164,
    -0.68896854,
    0.34479606,
    0.99089825,
    0.90510124,
    0.05941692,
    -0.50955284,
    -0.4415106,
    -1.2831217,
    -0.97601056,
    -0.8631987,
    -0.20892455,
    -0.75021267,
    -1.2193079,
    0.3239967,
    -0.026010232,
    -0.039476912,
    -0.12664902,
    0.009535544,
    0.08939319,
    0.21373276,
    -0.19362865,
    0.22576427,
    -0.0027303232,
    -0.077628,
    -0.11106214,
    0.00735133,
    1.9378375,
    1.5150657,
    1.7657595,
    -0.025828417,
    0.5442169,
    1.6175965,
    0.67236245,
    1.4253153,
    0.8264834,
    0.49127612,
    0.57544136,
    -0.038903136,
    0.53424984,
    2.821259,
    0.94532365,
    3.0151558,
];
