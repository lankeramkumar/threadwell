//! Refunds. A refund may not exceed the amount still refundable on the original charge.

/// Returns the amount refunded, or an error when the refund is larger than what is left.
pub fn refund(original_cents: u64, already_refunded_cents: u64, requested_cents: u64) -> Result<u64, String> {
    let refundable = original_cents.saturating_sub(already_refunded_cents);
    if requested_cents > refundable {
        return Err(format!("only {refundable} cents can still be refunded"));
    }
    Ok(requested_cents)
}
