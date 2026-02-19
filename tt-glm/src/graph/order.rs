//! Manual order management.

use crate::Result;

/// Position for inserting a new task.
#[derive(Debug, Clone, Copy)]
pub enum OrderPosition {
    /// Insert at the end (no specific position)
    End,
    /// Insert after a specific task
    After(i64),
    /// Insert before a specific task
    Before(i64),
    /// Insert between two tasks
    Between(i64, i64),
}

/// Calculate the manual_order value for a new task.
///
/// # Arguments
/// * `position` - Where to insert the task
/// * `order_after` - manual_order of the task to insert after (if applicable)
/// * `order_before` - manual_order of the task to insert before (if applicable)
/// * `max_order` - Current maximum manual_order in the system
pub fn calculate_order(
    position: OrderPosition,
    order_after: Option<f64>,
    order_before: Option<f64>,
    max_order: Option<f64>,
) -> Result<f64> {
    match position {
        OrderPosition::End => Ok(max_order.unwrap_or(0.0) + 10.0),
        OrderPosition::After(_) => {
            let after = order_after.unwrap_or(0.0);
            Ok(after + 10.0)
        }
        OrderPosition::Before(_) => {
            let before = order_before.unwrap_or(10.0);
            Ok(before - 10.0)
        }
        OrderPosition::Between(_, _) => {
            let after = order_after.unwrap_or(0.0);
            let before = order_before.unwrap_or(10.0);

            let midpoint = (after + before) / 2.0;

            // Check for precision exhaustion
            if midpoint == after || midpoint == before {
                return Err(crate::error::Error::PrecisionExhausted);
            }

            Ok(midpoint)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_order_end() {
        let order = calculate_order(OrderPosition::End, None, None, None).unwrap();
        assert_eq!(order, 10.0);

        let order = calculate_order(OrderPosition::End, None, None, Some(50.0)).unwrap();
        assert_eq!(order, 60.0);
    }

    #[test]
    fn test_calculate_order_after() {
        let order = calculate_order(OrderPosition::After(1), Some(20.0), None, None).unwrap();
        assert_eq!(order, 30.0);
    }

    #[test]
    fn test_calculate_order_before() {
        let order = calculate_order(OrderPosition::Before(2), None, Some(30.0), None).unwrap();
        assert_eq!(order, 20.0);
    }

    #[test]
    fn test_calculate_order_between() {
        let order =
            calculate_order(OrderPosition::Between(1, 2), Some(10.0), Some(20.0), None).unwrap();
        assert_eq!(order, 15.0);
    }

    #[test]
    fn test_precision_exhaustion() {
        // When values are very close, midpoint may equal one of them
        let a: f64 = 1.0;
        let b = f64::from_bits(a.to_bits() + 1); // Next representable float

        let result = calculate_order(OrderPosition::Between(1, 2), Some(a), Some(b), None);

        // This should fail due to precision exhaustion
        assert!(matches!(
            result,
            Err(crate::error::Error::PrecisionExhausted)
        ));
    }
}
