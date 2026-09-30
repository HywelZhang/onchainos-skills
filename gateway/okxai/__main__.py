"""包入口：`python -m okxai <verb>` / `python -m okxai serve`。"""

from .cli import main

if __name__ == "__main__":
    raise SystemExit(main())
