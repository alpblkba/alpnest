from pathlib import Path

DATA_HOME = Path.home() / ".local" / "share" / "alpnest"
CONFIG_HOME = Path.home() / ".config" / "alpnest"

RAW_MAIL_DIR = DATA_HOME / "raw" / "mail" / "messages"
STORE_DIR = DATA_HOME / "store"
GENERATED_DIR = DATA_HOME / "generated"
LOG_DIR = DATA_HOME / "logs"

MESSAGES_JSON = STORE_DIR / "messages.json"
EVENTSTREAMS_JSON = STORE_DIR / "eventstreams.json"
TASKS_JSON = STORE_DIR / "tasks.json"

MAIL_MD = GENERATED_DIR / "mail.md"
MAIL_DECOMPOSITION_MD = GENERATED_DIR / "mail_decomposition.md"
MAIL_SYNC_STATE_JSON = STORE_DIR / "mail_sync_state.json"
TODAY_MD = GENERATED_DIR / "today.md"
CALENDAR_MD = GENERATED_DIR / "calendar.md"

