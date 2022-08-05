# Nautirust

If you want to use nautirust but not develop for it, take a look at [nautirust-configs](https://github.com/ajuvercr/nautirust-configs).

Nautirust is a command-line program that helps configuring, and is able to start, a data processing pipeline of processes (called steps).
It does not restrict on programming language or start-up sequence.

Nautirust consists of 3 concepts: 1. a step, 2. a runner and 3. a channel. A pipeline is a sequence of steps.

Runners are designed to start a step and can require steps to define a configuration. 
For example a runner that executes a step written in javascript requires the location of the main source file and the name of the function that will be executed.

Each step is programming language independent, but has to specify what runner must be used to start the step. 
A step also specifies what arguments should be provided to that step, including the type.

The type of these arguments is loose but there are two special cases: stream reader and stream writer. 
Readers and writers connect two processes together with _a_ channel. Nautirust helps connecting these channels.

The goal of the runner is not only to start up a step, but also to abstract away the underlying channel. This eases the implementation of steps.
Runners specify what channels they support. Not all runners can support all channels, nautirust uses this information to only suggest plausible channels.

## Usage

Example step file (for more details see later)
```json
{
  "id": "helloWorld",
  "runnerId": "JsRunner",
  "config": {
    "jsFile": "main.js",
    "methodName": "sayHello"
  },
  "args": [
    {
      "id": "to",
      "type": "string"
    }
  ]
}
```

Generate a plan and save it to plan.json:
```
nautirust generate -o plan.json [...steps]
```

Execute the plan:
```
nautirust run plan.json
```

## Configuration

Channels and runners have to be defined, this can be done with command line arguments or a config file (`orchestrator.toml` or specify with the `--config` flag).
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
It also defines some options for the nautirust. The user can choose between these options.


### Runner configuration

Example runner configuration:
```json
  {
    "id": "JsRunner",
    "runnerScript": "node ./lib/index.js {config} {cwd}",
    "canUseChannel": [
      "file", "ws"
    ],
    "canUseSerialization": [
      "json", "turtle", "plain"
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
- `canUseSerialization`: what serializations are supported

When a runner is configured in a step `jsFile` and `methodName` have to be provided.


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
      "type": "any",
      "description": "config argument"
    },
    {
      "id": "a_default_value",
      "type": "string",
      "default": true,
      "value": "test"
    },
    {
      "id": "a_suggested_value",
      "type": "string",
      "default": false,
      "value": "test2"
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
Default arguments are also possible, see `a_default_value`. However if default is `false` and a value is still given, it will be suggested to the user instead.


## Functionality

```sh
$ nautirust -h
nautirust 0.1.1

USAGE:
    nautirust [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -c, --channels <CHANNELS>    Glob to indicate channels locations
        --config <CONFIG>        Location of a config file
    -h, --help                   Print help information
    -r, --runners <RUNNERS>      Glob to indicate runners locations
    -V, --version                Print version information

SUBCOMMANDS:
    docker      Create a docker-compose file from a nautirust pipeline
    generate    Generate a pipeline of steps
    help        Print this message or the help of the given subcommand(s)
    prepare     Prepares the execution pipeline by starting the required channels/runner
    run         Run a configured pipeline
    stop        Gracefully stop the runners and channels specified in the config
    validate    Validate configureations for runners and channels
```

You can specify (by glob) where to find channels and runners with `--channels` and `--runners` respectively.
Nautirust also supports toml file that specifies these properties (`--config`).


### generate

```sh
$ nautirust generate -h
nautirust-generate 
Generate a pipeline of steps

USAGE:
    nautirust generate [OPTIONS] [STEPS]...

ARGS:
    <STEPS>...    Steps to include in the pipeline (ordered)

OPTIONS:
    -a, --automatic          Try infer basic configurations details
    -h, --help               Print help information
    -o, --output <OUTPUT>    Output location of the generated pipeline file
```

Nautirust takes multiple steps to create a pipeline configuration file.
Interactively the user is asked questions about the required arguments, taking special care of stream readers and stream writers. 

These questions can add up, use the `-a` flag to let nautirust infer some basic configuration consisting of:
- automatic linking of stream readers and writers with the same name
- automatically choosing a channel configuration when the channel type is specified

`-o` takes a filename to store the generated configuration (default is stdout).

### run
```sh
$ nautirust run -h
nautirust-run 
Run a configured pipeline

USAGE:
    nautirust run [OPTIONS] <FILE>

ARGS:
    <FILE>    Config file

OPTIONS:
    -h, --help                 Print help information
    -t, --tmp-dir <TMP_DIR>    temporary directory to put step configuration files
```

Nautirust runs a generated configuration file.

Each runners takes in a configuration file that specifies the steps that should be executed, with `-t` you can specify the location of these configuration files.


### prepare
```sh
$ nautirust prepare -h
nautirust-prepare 
Prepares the execution pipeline by starting the required channels/runner

USAGE:
    nautirust prepare <FILE>

ARGS:
    <FILE>    Config file

OPTIONS:
    -h, --help    Print help information
```

Nautirust takes a generated configuration file, and prepares the used steps, runners and channels.
This can be used to run a build script, start a docker-compose instance, ...


### stop
```sh
$ nautirust stop -h
nautirust-stop 
Gracefully stop the runners and channels specified in the config

USAGE:
    nautirust stop <FILE>

ARGS:
    <FILE>    Config file

OPTIONS:
    -h, --help    Print help information
```

Same as prepare, but in reverse.


### docker

**EXPERIMENTAL**
```sh
$ nautirust docker -h
nautirust-docker 
Create a docker-compose file from a nautirust pipeline

USAGE:
    nautirust docker [OPTIONS] <FILE>

ARGS:
    <FILE>    Config file

OPTIONS:
    -h, --help                 Print help information
    -o, --output               
    -t, --tmp-dir <TMP_DIR>    temporary directory to put step configuration files
```

Does the same as `nautirust run` but generates a docker-compose file that when executed start the pipeline.
This only works when the runners have specified a docker command.
This docker command is expected to generate a Dockerfile somewhere that executes the step.
This command also prints to stdout the required contents for this step (at least specify the generated Dockerfile and set up the correct docker context).

The used channels also return a part of the docker-compose file (if anything).


### validate

```sh
$ nautirust validate -h
nautirust-validate 
Validate configureations for runners and channels

USAGE:
    nautirust validate

OPTIONS:
    -h, --help    Print help information
```

Validates the specified channels and runners.

