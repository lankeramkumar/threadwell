"""Retry policy for the payment provider.

Delays grow exponentially: 0.5 s, 1 s, 2 s, and so on, capped at 30 seconds.
"""

BASE_DELAY_SECONDS = 0.5
MAX_DELAY_SECONDS = 30.0


def retry_delay(attempt: int) -> float:
    """Seconds to wait before retry number `attempt` (starting at 1)."""
    delay = BASE_DELAY_SECONDS * (2 ** (attempt - 1))
    return min(delay, MAX_DELAY_SECONDS)
