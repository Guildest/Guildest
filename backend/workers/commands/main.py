from backend.common.config import AppConfig
from backend.common.models import QueueMessage
from backend.workers.consumer import run_worker


async def handle_message(message: QueueMessage, _config: AppConfig, _db: object) -> None:
    if message.event != "MESSAGE_CREATE":
        return
    # Prefix commands are deprecated; use Discord app commands instead.
    return


def main() -> None:
    run_worker("commands", handle_message)


if __name__ == "__main__":
    main()
