//! Draft order rules.

/// Who picks at the global, 0-based `pick_index`: returns the 0-based
/// round and the index into the league's members in draft order. Snake
/// drafts reverse the order on every odd round.
pub fn pick_slot(pick_index: i32, num_members: i32, snake: bool) -> (i32, usize) {
    let round = pick_index / num_members;
    let in_round = pick_index % num_members;
    let slot = if snake && round % 2 == 1 {
        num_members - 1 - in_round
    } else {
        in_round
    };
    (round, slot as usize)
}

#[cfg(test)]
mod tests {
    use super::pick_slot;

    #[test]
    fn snake_reverses_odd_rounds() {
        let order: Vec<usize> = (0..9).map(|i| pick_slot(i, 3, true).1).collect();
        assert_eq!(order, vec![0, 1, 2, 2, 1, 0, 0, 1, 2]);
    }

    #[test]
    fn linear_repeats_order() {
        let order: Vec<usize> = (0..6).map(|i| pick_slot(i, 3, false).1).collect();
        assert_eq!(order, vec![0, 1, 2, 0, 1, 2]);
        assert_eq!(pick_slot(5, 3, false).0, 1);
    }
}
