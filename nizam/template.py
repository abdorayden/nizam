import json


class CommentedNizamTemplate(dict):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._comments = {}

    def set_comment(self, key, comment):
        """Attach a comment to a key.

        ``key`` is either a top-level key (``"tasks"``) or a path to a nested
        key/list item (``("tasks", "job_0", "cmd", 0, "run")``).
        """
        if isinstance(key, str):
            key = (key,)
        self._comments[tuple(key)] = comment

    def dump(self, indent=2):
        return self._dump_value(self, (), 0, indent)

    def _dump_value(self, value, path, level, indent):
        pad = " " * (indent * level)
        inner_pad = " " * (indent * (level + 1))

        if isinstance(value, dict):
            if not value:
                return "{}"
            lines = ["{"]
            items = list(value.items())
            for i, (key, child) in enumerate(items):
                child_path = path + (key,)
                child_str = self._dump_value(child, child_path, level + 1, indent)
                comma = "," if i < len(items) - 1 else ""
                comment = self._comments.get(child_path)
                suffix = f" // {comment}" if comment else ""
                lines.append(
                    f"{inner_pad}{json.dumps(key)}: {child_str}{comma}{suffix}"
                )
            lines.append(f"{pad}}}")
            return "\n".join(lines)

        if isinstance(value, list):
            if not value:
                return "[]"
            lines = ["["]
            for i, child in enumerate(value):
                child_path = path + (i,)
                child_str = self._dump_value(child, child_path, level + 1, indent)
                comma = "," if i < len(value) - 1 else ""
                comment = self._comments.get(child_path)
                suffix = f" // {comment}" if comment else ""
                lines.append(f"{inner_pad}{child_str}{comma}{suffix}")
            lines.append(f"{pad}]")
            return "\n".join(lines)

        return json.dumps(value)


TEMPLATE = CommentedNizamTemplate(
    {
        "limite_tasks": 3,
        "exit_on_first_error": True,
        "bind": {
            "delay": "0.2",
        },
        "tasks": {
            "job_0": {
                "cmd": [
                    {
                        "when": {
                            "os": "linux",
                        },
                        "run": "echo linux ",
                    },
                    {
                        "when": {
                            "os": "windows",
                        },
                        "run": "echo windows ",
                    },
                    {
                        "run": "echo 'unknown platform' ",
                    },
                ],
            },
            "job_1": {
                "cmd": "sleep ^delay^",
            },
        },
    }
)

TEMPLATE.set_comment("limite_tasks", "maximum number of tasks that run at the same time")
TEMPLATE.set_comment("exit_on_first_error", "stop the whole run as soon as a task fails")
TEMPLATE.set_comment(
    "bind",
    "named values reused in commands with ^name^ (built-in bindings: HOME and PWD)",
)
TEMPLATE.set_comment("tasks", "object whose keys are task ids and whose values are task objects")

TEMPLATE.set_comment(
    ("tasks", "job_0"),
    'first task; add "deps": ["other_task"] to wait for other tasks',
)
TEMPLATE.set_comment(
    ("tasks", "job_1"),
    "second task; it has no deps, so it can run in parallel with job_0",
)

TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd"),
    "cmd is either one command string or a list of conditional commands",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 0),
    "commands are checked in order; the first matching condition is used",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 0, "when"),
    "condition that must match before this command is selected",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 0, "when", "os"),
    "built-in operating-system condition (linux, windows, macos, ...)",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 0, "run"),
    "command to execute when the condition matches",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 1, "when"),
    "another platform branch; only one branch is selected",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 1, "run"),
    "command to execute on Windows",
)
TEMPLATE.set_comment(
    ("tasks", "job_0", "cmd", 2),
    "entry without when is the fallback for unsupported platforms",
)

TEMPLATE.set_comment(
    ("tasks", "job_1", "cmd"),
    "a single command string; ^delay^ is replaced by the bind value before running",
)
