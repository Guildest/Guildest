import logging


def configure_logging(level: str = "INFO") -> None:
    """Configure application-wide logging format and level."""

    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )
