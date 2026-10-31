<div align="center">
  <img src="./assets/logo.png" alt="Nizam logo" width="520">
  <h1>Nizam</h1>
</div>

Nizam is a small task runner for commands that need to run in a particular order. Describe tasks and their dependencies in a JSON file, and Nizam works out which tasks can run together. It can also cap parallel work, choose commands for different operating systems, and reuse tasks and settings from other templates.

“Nizam” is an Arabic word meaning “system” or “order.”

## How it works

Nizam reads a template, resolves its bindings and inherited templates, then checks the task dependencies. It groups ready tasks into levels: tasks in one level can run in parallel, while each later level waits for the previous one to finish. The runner captures each command's output and exit status and prints a summary.

## Requirements

- Python 3.8 or newer and pip, for building and using the Python extension
- Rust, installed with rustup (including `cargo` and `rustc`)
- Maturin 1.5 or newer and earlier than 2.0, for building the Python extension

The Rust command-line runner only needs the Rust toolchain.

## Build and run

### Rust command-line runner

Run one template directly:

```sh
cargo run -- templates/03_diamond.json
```

You can pass more than one template, or a directory containing JSON templates. With no paths, Nizam reads the JSON files directly inside `templates/`:

```sh
cargo run
cargo run -- templates/failures
cargo run -- templates/01_parallel_echo.json templates/02_dependency_chain.json
```

The command exits with a non-zero status if a template cannot be loaded or scheduled, or if any task fails. Commands run by the templates have the same permissions as the user who started Nizam, so review a template before running it.

### Python extension

Create a virtual environment, install Maturin, and build the extension in development mode:

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install 'maturin>=1.5,<2.0'
maturin develop
```

Then use the Python API:

```python
from nizam import Engine

execution = Engine("templates/03_diamond.json").run()

print(execution.order)       # task IDs in dependency order
print(execution.levels)      # task IDs grouped by parallel level
print(execution.succeeded)   # whether every task completed successfully

for task in execution.results:
    print(task.cmd, task.status, task.succeeded)
    if task.out:
        print(task.out)
    if task.err:
        print(task.err)
```

The result for each task also has a `never_run` property, which is true if Nizam could not start that command. A command that starts and exits unsuccessfully has `succeeded == False` and a status code when the operating system provides one.

The repository also includes `nizam.sh test` as a shortcut for running the Rust test suite. Use the direct `cargo run` and Maturin commands above to run templates or control the Python build setup.

## Writing a template

Each template is a JSON object with a `tasks` map. Task IDs are keys; each task has a command and may list task IDs it depends on:

```json
{
  "limite_tasks": 2,
  "exit_on_first_error": true,
  "bind": {
    "source": "main.c",
    "program": "app"
  },
  "tasks": {
    "compile": {
      "cmd": "cc -c ^source^ -o main.o"
    },
    "link": {
      "cmd": "cc main.o -o ^program^",
      "deps": ["compile"]
    },
    "run": {
      "cmd": "./^program^",
      "deps": ["link"]
    }
  }
}
```

Here, `compile` runs first, then `link`, then `run`. Tasks without dependencies can run together. `limite_tasks` caps how many tasks Nizam runs at once; if it is omitted, all ready tasks may run concurrently. The spelling `limite_tasks` is part of the current template format.

### Template options

- `tasks` (required): object mapping task IDs to task definitions.
- `cmd` (required per task): a command string or a list of conditional commands.
- `deps` (optional per task): task IDs that must finish before this task starts. An unknown ID or a dependency cycle makes the template invalid.
- `limite_tasks` (optional): maximum parallel tasks. Non-positive values are ignored with a warning.
- `exit_on_first_error` (optional, defaults to `false`): when true, Nizam finishes the current batch and stops before starting further batches or levels after a failure. Otherwise, later dependent tasks still run after a failed dependency.
- `bind` (optional): named text values substituted into commands with `^name^`. The built-in `^HOME^` and `^PWD^` values are also available. Unknown binding names remain as written.
- `uses` (optional): paths to other template files, resolved relative to the current template. Their tasks and bindings are inherited; local definitions override inherited ones. Inheritance can be chained, but a `uses` cycle is invalid.

Commands can be selected by operating system. Conditions are checked in order; the first matching entry runs. An entry without `when` is the fallback:

```json
{
  "cmd": [
    { "when": { "os": "linux" }, "run": "echo Running on Linux" },
    { "when": { "os": "windows" }, "run": "echo Running on Windows" },
    { "run": "echo Running on another system" }
  ]
}
```

The current condition supported is `os`, matched against Rust's platform name (for example, `linux`, `windows`, or `macos`). Bindings are substituted after the matching command is selected.

Nizam accepts `//` comments in template files. Keep comments on their own lines: the loader strips text from the first `//` on each line, including if those characters appear inside a JSON string.

## Examples

The `templates/` directory includes examples for independent parallel tasks, dependency chains, a diamond-shaped dependency graph, concurrency limits, OS-specific commands, and failure handling. For example:

```sh
cargo run -- templates/04_concurrency.json
cargo run -- templates/failures/continue_after_failure.json
```

## Tests

Run the Rust unit and integration tests with:

```sh
cargo test
```

The tests cover template loading and inheritance, bindings and conditions, dependency ordering and cycle detection, command execution, and engine failure handling.

## Project layout

- `src/json.rs` loads and resolves templates.
- `src/task_manager.rs` validates task dependencies.
- `src/kahn.rs` computes task order and parallel levels.
- `src/process.rs` starts commands and collects their results.
- `src/engine.rs` coordinates validation, scheduling, and execution.
- `src/main.rs` implements the Rust command-line runner.
- `src/lib.rs` exposes the Python extension through PyO3.
- `nizam/` contains the Python-side template helper and entry point.
- `tests/` and `templates/` contain the test suite and runnable examples.
