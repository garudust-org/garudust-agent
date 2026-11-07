use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::AgentError;

pub struct IterationBudget {
    remaining: AtomicU32,
    max: u32,
}

impl IterationBudget {
    pub fn new(max: u32) -> Self {
        Self {
            remaining: AtomicU32::new(max),
            max,
        }
    }

    pub fn consume(&self) -> Result<u32, AgentError> {
        let mut curr = self.remaining.load(Ordering::SeqCst);
        loop {
            if curr == 0 {
                return Err(AgentError::BudgetExhausted(self.max));
            }
            match self.remaining.compare_exchange_weak(
                curr,
                curr - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(curr - 1),
                Err(actual) => curr = actual,
            }
        }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining.load(Ordering::SeqCst)
    }

    pub fn max(&self) -> u32 {
        self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_max() {
        let b = IterationBudget::new(3);
        assert_eq!(b.remaining(), 3);
        assert_eq!(b.max(), 3);
    }

    #[test]
    fn consume_decrements() {
        let b = IterationBudget::new(3);
        assert_eq!(b.consume().unwrap(), 2);
        assert_eq!(b.remaining(), 2);
    }

    #[test]
    fn consume_until_exhausted() {
        let b = IterationBudget::new(2);
        b.consume().unwrap();
        b.consume().unwrap();
        assert!(b.consume().is_err());
        assert_eq!(b.remaining(), 0);
    }

    #[test]
    fn zero_budget_immediately_exhausted() {
        let b = IterationBudget::new(0);
        assert!(b.consume().is_err());
    }
}
