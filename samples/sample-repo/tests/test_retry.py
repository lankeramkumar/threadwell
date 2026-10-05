from src.retry import retry_delay


def test_first_retry_waits_half_a_second():
    assert retry_delay(1) == 0.5


def test_delay_is_capped():
    assert retry_delay(20) == 30.0
