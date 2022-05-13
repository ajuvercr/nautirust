# nautirust

An orchestrator for the connector architecture.
This architecture defines processors that take input.
Input can be many things, mostly configuration stuff, streamReaders and streamWriters.

Readers and writers are connectors that use a channel to forward messages (currently all is json).
What channel is used, usually doesn't really matter, the orchestrator asks the user what channel can be used.

Processors are executed with runners, the task of a runner is to start up or configure the content around the processor
that the processor will use the specified connector.

## Usage

Generate a plan:
```
cargo run -- generate -o plan.json [...steps]
```

Execute the plan:
```
cargo run -- run plan.json
```

## Configuration

Channels and runners have to be defined, this can be done with command line arguments or a config file.

```toml
channels = "configs/channels/*.json"
runners = "configs/runners/*/runner.json"
```

This configuration file will look for any channel defined inside `configs/channels` and will
look for runners defined in `configs/runners` that have a `runner.json` file.

### Channel configuration

Example channel configuration:
```json
{
  "id": "file",
  "requiredFields": [
    "path",
    "onReplace"
  ],
  "options": [
    {"path": "test1.json", "onReplace": true},
    {"path": "test2.json", "onReplace": false},
    {"path": "test3.json"}
  ]
}
```

Here a channel is defined called `file` and specifies two fields are required: `path` and `onReplace`.
It also defines some options for the orchestrator and user to choose from.

### Runner configuration

Example runner configuration:
```json
  {
    "id": "JsRunner",
    "runnerScript": "node ./lib/index.js {config} {cwd}",
    "canUseChannel": [
      "file", "ws"
    ],
    "requiredFields": [
      "jsFile",
      "methodName"
    ]
  }
```

Here a runner called JsRunner is defined. Required fields are
- `runnerScript`: how is the runner started
- `canUseChannel`: what channels can this runner provide to the processor

Runners can also be configured by a step. Here it requires a `jsFile` and a `methodName`.


### Step configuration

Example step configuration:
```json
{
  "id": "readCsv",
  "runnerId": "JsRunner",
  "config": {
    "jsFile": "main.js",
    "methodName": "writeTo"
  },
  "args": [
    {
      "id": "config",
      "type": "any"
    },
    {
      "id": "writer",
      "type": "streamWriter",
      "targetIds": [
        "data"
      ]
    }
  ]
}
```

A step is a processor. The processor specifies that it can be executed with the `JsRunner` runner.
It also specifies what arguments have to be defined before being able to execute.

