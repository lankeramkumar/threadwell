# Decisions

- 2026-03-02: Every charge request carries an idempotency key. Retries reuse the key.
- 2026-03-02: Refunds are capped at the amount still refundable on the original charge.
- 2026-04-10: The retry delay is capped at 30 seconds. Longer waits were rejected by the support team.
