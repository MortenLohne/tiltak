#[cfg(test)]
use crate::position::Komi;
use crate::{
    evaluation::parameters::{Policy, PolicyApplier},
    position::{GroupData, Move, Position},
};
use board_game_traits::{Color, GameResult, Position as _};

pub struct ProofTree<const S: usize> {
    position: Position<S>,
    root: OrNode<S>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofResult {
    Proved,
    Disproved,
}

impl<const S: usize> ProofTree<S> {
    pub fn new(position: Position<S>) -> Self {
        Self {
            position,
            root: OrNode::new(),
        }
    }

    pub fn select(&mut self) {
        self.root.select(&mut self.position);
    }

    pub fn result(&self) -> Option<ProofResult> {
        if self.root.proof_numbers.proof_number == 0 {
            Some(ProofResult::Proved)
        } else if self.root.proof_numbers.disproof_number == 0 {
            Some(ProofResult::Disproved)
        } else {
            None
        }
    }

    pub fn pv(&self) -> Vec<Move<S>> {
        let mut pv = Vec::new();
        self.root.pv(&mut pv);
        pv
    }

    fn root_proof_numbers(&self) -> Vec<(Move<S>, ProofNumbers)> {
        self.root
            .children
            .iter()
            .map(|(move_, child)| (*move_, child.proof_numbers.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProofNumbers {
    proof_number: u32,
    disproof_number: u32,
}

impl ProofNumbers {
    fn new() -> Self {
        Self {
            proof_number: 1,
            disproof_number: 1,
        }
    }

    fn loss_draw() -> Self {
        Self {
            proof_number: u32::MAX,
            disproof_number: 0,
        }
    }

    fn win() -> Self {
        Self {
            proof_number: 0,
            disproof_number: u32::MAX,
        }
    }

    fn and(self, other: ProofNumbers) -> Self {
        let proof_number = self.proof_number.saturating_add(other.proof_number);
        let disproof_number = self.disproof_number.min(other.disproof_number);
        Self {
            proof_number,
            disproof_number,
        }
    }

    fn or(self, other: ProofNumbers) -> Self {
        let proof_number = self.proof_number.min(other.proof_number);
        let disproof_number = self.disproof_number.saturating_add(other.disproof_number);
        Self {
            proof_number,
            disproof_number,
        }
    }
}

struct AndNode<const S: usize> {
    children: Box<[(Move<S>, OrNode<S>)]>,
    proof_numbers: ProofNumbers,
}

impl<const S: usize> AndNode<S> {
    fn new() -> Self {
        Self {
            children: Box::new([]),
            proof_numbers: ProofNumbers::new(),
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn select(&mut self, position: &mut Position<S>) {
        if self.children.is_empty() {
            return self.expand(position);
        }
        let (mv, child) = self
            .children
            .iter_mut()
            .min_by_key(|(_mv, child)| child.proof_numbers.disproof_number)
            .unwrap();
        let mv = *mv;
        let reverse_move = position.do_move(mv);
        child.select(position);
        position.reverse_move(reverse_move);

        self.proof_numbers = self
            .children
            .iter()
            .map(|(_, child)| child.proof_numbers.clone())
            .reduce(|e, acc| e.and(acc))
            .unwrap();
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn expand(&mut self, position: &mut Position<S>) {
        let mut legal_moves = vec![];
        position.generate_moves(&mut legal_moves);
        let mut child_nodes = Vec::with_capacity(legal_moves.len());
        for mv in legal_moves {
            child_nodes.push((mv, OrNode::new()));
        }
        self.children = child_nodes.into_boxed_slice();
    }

    fn pv(&self, pv: &mut Vec<Move<S>>) {
        if self.children.is_empty() {
            return;
        }

        let (best_move, best_child) = self
            .children
            .iter()
            .max_by_key(|(_mv, child)| child.visits)
            .unwrap();
        pv.push(*best_move);
        best_child.pv(pv);
    }
}

struct OrNode<const S: usize> {
    children: Box<[(Move<S>, AndNode<S>)]>,
    proof_numbers: ProofNumbers,
    visits: u32,
}

impl<const S: usize> OrNode<S> {
    fn new() -> Self {
        Self {
            children: Box::new([]),
            proof_numbers: ProofNumbers::new(),
            visits: 0,
        }
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn select(&mut self, position: &mut Position<S>) {
        self.visits += 1;
        if self.children.is_empty() {
            return self.expand(position);
        }

        let (mv, child) = self
            .children
            .iter_mut()
            .min_by_key(|(_mv, child)| child.proof_numbers.proof_number)
            .unwrap();

        let mv = *mv;
        let reverse_move = position.do_move(mv);
        child.select(position);
        position.reverse_move(reverse_move);

        self.proof_numbers = self
            .children
            .iter()
            .map(|(_, child)| child.proof_numbers.clone())
            .reduce(|e, acc| e.or(acc))
            .unwrap();
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn expand(&mut self, position: &mut Position<S>) {
        assert!(self.children.is_empty());
        let mut moves = vec![];
        position.generate_moves(&mut moves);

        if moves.iter().any(|mv| {
            let reverse_move = position.do_move(*mv);
            if position.relative_result() == Some(RelativeResult::Win) {
                position.reverse_move(reverse_move);
                return true;
            }
            position.reverse_move(reverse_move);
            false
        }) {
            self.proof_numbers = ProofNumbers::win();
            return;
        }
        position.filter_tak_threat_moves(&mut moves);
        if moves.is_empty() {
            self.proof_numbers = ProofNumbers::loss_draw();
            return;
        }

        let mut child_nodes = Vec::with_capacity(moves.len());
        for mv in moves {
            child_nodes.push((mv, AndNode::new()))
        }
        self.children = child_nodes.into_boxed_slice();
    }

    fn pv(&self, pv: &mut Vec<Move<S>>) {
        if self.children.is_empty() {
            return;
        }

        let (best_move, best_child) = self
            .children
            .iter()
            .min_by_key(|(_mv, child)| child.proof_numbers.proof_number)
            .unwrap();

        pv.push(*best_move);
        best_child.pv(pv);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeResult {
    Win,
    Loss,
    Draw,
}

impl<const S: usize> Position<S> {
    /// Game result relative to the side who made the last move
    fn relative_result_with_group_data(&self, group_data: &GroupData<S>) -> Option<RelativeResult> {
        let game_result = self.game_result_with_group_data(group_data)?;
        match (game_result, !self.side_to_move()) {
            (GameResult::WhiteWin, Color::White) => Some(RelativeResult::Win),
            (GameResult::BlackWin, Color::Black) => Some(RelativeResult::Win),
            (GameResult::WhiteWin, Color::Black) => Some(RelativeResult::Loss),
            (GameResult::BlackWin, Color::White) => Some(RelativeResult::Loss),
            (GameResult::Draw, _) => Some(RelativeResult::Draw),
        }
    }

    fn relative_result(&self) -> Option<RelativeResult> {
        self.relative_result_with_group_data(&self.group_data())
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    /// Vector must already be filled with all legal moves
    fn filter_tak_threat_moves(&mut self, moves: &mut Vec<Move<S>>) {
        moves.retain(|mv| {
            let reverse_move = self.do_move(*mv);
            match self.relative_result() {
                Some(RelativeResult::Win) => println!("Warning: Should not see immediate win here"),
                Some(RelativeResult::Draw | RelativeResult::Loss) => {
                    self.reverse_move(reverse_move);
                    return false;
                }
                None => (),
            }
            self.null_move();

            let makes_tak_threat = self.has_winning_move();

            self.null_move();
            self.reverse_move(reverse_move);
            makes_tak_threat
        });
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn has_winning_move(&mut self) -> bool {
        let mut simple_moves = vec![];
        let mut moves = vec![];
        let mut fcd_per_move = vec![];
        let mut feature_sets = vec![];

        self.generate_moves_with_probabilities::<Policy<S>>(
            &self.group_data(),
            &mut simple_moves,
            &mut moves,
            &mut fcd_per_move,
            <Position<S>>::policy_params(self.komi()),
            &mut feature_sets,
        );

        let applier = feature_sets[0].clone();
        if applier.has_immediate_win() {
            return true;
        }

        moves.sort_by(|(_, score), (_, score2)| score.partial_cmp(score2).unwrap().reverse());
        let child_moves: Vec<Move<S>> = moves.into_iter().map(|(mv, _)| mv).collect();
        // let mut child_moves = vec![];
        // self.generate_moves(&mut child_moves);
        child_moves
            .into_iter()
            .any(|child_move| self.move_wins(child_move))
    }

    // Never inline, for profiling purposes
    #[inline(never)]
    fn move_wins(&mut self, mv: Move<S>) -> bool {
        let reverse_child_move = self.do_move(mv);
        let relative_result = self.relative_result();
        let result = match relative_result {
            Some(RelativeResult::Win) => true,
            Some(RelativeResult::Draw | RelativeResult::Loss) => false,
            None => false,
        };
        self.reverse_move(reverse_child_move);
        result
    }
}

#[test]
fn create_tak_threats_test() {
    let mut position: Position<6> = Position::from_fen_with_komi(
        "x5,1/x6/x2,1,1,1,1/x2,2,2C,1,x/x3,2,2,2/2,x5 1 7",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    let old_position = position.clone();
    let mut moves = Vec::new();
    position.generate_moves(&mut moves);
    position.filter_tak_threat_moves(&mut moves);
    assert_eq!(
        moves.len(),
        4,
        "{}",
        moves
            .iter()
            .map(|mv| mv.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    );
    assert_eq!(position, old_position);
}

#[test]
fn prove_test() {
    let position: Position<6> = Position::from_fen_with_komi(
        "x5,1/x6/x2,1,1,1,1/x2,2,2C,1,x/x3,2,2,2/2,x5 1 7",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    let mut tree = ProofTree::new(position);
    tree.select();
    assert_eq!(tree.result(), None);
}

#[cfg(test)]
fn find_tinue_prop<const S: usize>(position: Position<S>, result: ProofResult) {
    let mut tree = ProofTree::new(position);
    for _ in 0..100_000 {
        if tree.result().is_some() {
            break;
        }
        tree.select();
    }
    for (mv, proof_numbers) in tree.root_proof_numbers() {
        println!("{}: {:?}", mv, proof_numbers);
    }
    println!(
        "Result {:?}, pv: {}",
        tree.result(),
        tree.pv()
            .iter()
            .map(|mv| mv.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    );
    assert_eq!(tree.result(), Some(result));
}

#[test]
fn disprove_move2_tinue_test() {
    let position: Position<6> = Position::from_fen_with_komi(
        "x5,1/x6/x6/x6/x6/2,x5 1 2",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    find_tinue_prop(position, ProofResult::Disproved);
}

#[test]
fn disprove_tinue_test2() {
    let position: Position<6> = Position::from_fen_with_komi(
        "2,x2,1,2,1/2,2,1,1,121S,1/2,2,2,1,12S,1/1,1,x,122221C,x,2/2,2,11212S,111112C,121S,x/1,1,2,1,1,2 2 35",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    find_tinue_prop(position, ProofResult::Disproved);
}

#[test]
fn prove_easy_tinue_test1() {
    let position: Position<6> = Position::from_fen_with_komi(
        "x5,1/x2,2,x2,1/x2,2,x2,1/x2,2,x,1,x/x2,2,x,1,x/2,x5 1 6",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    find_tinue_prop(position, ProofResult::Proved);
}

#[test]
fn prove_easy_tinue_test2() {
    let position: Position<6> = Position::from_fen_with_komi(
        "2,x2,1,2,1/2,2,1,1,121S,1/2,2,2,112S,x,1/121,x,22221C,1,x,2/122C,2112,x,11111,121S,1/1,1,21,x,1,2 2 39",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    find_tinue_prop(position, ProofResult::Proved);
}

#[test]
fn prove_tinue_test() {
    let position: Position<6> = Position::from_fen_with_komi(
        "2,x2,1,2,1/2,2,1,1,121S,1/2,2,2,112S,x,1/1,1,22221C,1,x,2/2,2,112122C,11111,121S,1/1,1,2,1,1,2 2 37",
        Komi::from_half_komi(4).unwrap(),
    )
    .unwrap();
    find_tinue_prop(position, ProofResult::Proved);
}
