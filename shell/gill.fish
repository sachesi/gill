# gill: send the lines fish cannot run to bash, and keep bash's state between them.
# Load it from config.fish:
#
#     if status is-interactive
#         gill init fish | source
#     end

set -g __gill_session $fish_pid-(random 1 2147483647)

# Returns 0 when the line belongs to bash. Both parsers decide; nothing is guessed.
function __gill_verdict --argument-names line
    # A continuation backslash makes fish wait for the next line. Bash would read it as
    # a literal backslash, and fish's parser calls it an error.
    set -l backslashes (string match -r -- '\\\\*\z' $line)
    test (math (string length -- "$backslashes") % 2) -eq 1
    and return 1

    commandline --input=$line --is-valid
    switch $status
        case 1
            return 0
        case 2
            # Bash reads these fish block openers as ordinary commands.
            string match -qr '^\s*(begin|switch)(\s|;|$)' -- $line
            and return 1
            command bash --noprofile --norc -O extglob -n -c $line 2>/dev/null
            return
    end

    set -l words (commandline --input=$line --tokens-expanded)
    if contains -- "$words[1]" source .; and set -q words[2]
        and test "$words[2]" != -
        and not string match -q -- '*.fish' $words[2]
        return 0
    end
    # fish's tokenizer reads `[[ ... ]]` as a single bracketed token.
    if string match -qr '^\s*\[\[\s' -- $line; and not type -q -- '[['
        return 0
    end
    # fish refuses a line with a bare NAME=value anywhere before running any of it,
    # which only its full parser reports.
    string match -q -- '*=*' $line; or return 1
    set -l fish (status fish-path)
    not command $fish --no-config --no-execute -c $line 2>/dev/null
end

# Runs a command in bash and applies what it changed. Takes gill run's arguments:
# `-c LINE` or `-- COMMAND [ARG...]`.
function __gill_bash
    command gill run --session $__gill_session $argv
    set -l run_status $status
    __gill_apply
    return $run_status
end

function __gill_apply
    set -l path $XDG_RUNTIME_DIR/gill/$__gill_session/records
    test -f $path; or return 0
    set -l fields (string split0 <$path)
    # Applied once only, even if the next run fails before replacing the file.
    true >$path

    set -l i 1
    while set -q fields[(math $i + 2)]
        set -l name $fields[(math $i + 1)]
        set -l value $fields[(math $i + 2)]
        switch $fields[$i]
            case set
                set -gx -- $name $value
            case erase
                set -eg -- $name
            case cd
                cd -- $value
            case commands
                __gill_define_commands (string split -n \n -- $value)
        end
        set i (math $i + 3)
    end
end

# The functions gill defined so fish can call bash functions and aliases.
set -g __gill_commands
set -g __gill_description 'Run the bash function or alias of this name'

# A fish function replaced by the user no longer carries gill's description, and gill
# must not remove it.
function __gill_owns --argument-names name
    functions -q -- $name; and functions -- $name | string match -q -- "*--description '$__gill_description'*"
end

# Gives each bash function and alias fish does not know itself a fish function that
# runs that one command in bash, and removes the ones bash no longer has.
function __gill_define_commands
    for name in $__gill_commands
        if not contains -- $name $argv; and __gill_owns $name
            functions -e -- $name
        end
    end
    set -l defined
    for name in $argv
        if contains -- $name $__gill_commands
            set -a defined $name
        else if not type -q -- $name
            function $name --description $__gill_description
                __gill_bash -- (status current-function) $argv
            end
            set -a defined $name
        end
    end
    set -g __gill_commands $defined
end

function __gill_run_line
    __gill_bash -c $__gill_line
end

# Runs the line in bash. What fish executes is a call to __gill_run_line, followed by
# the line as comments so it stays readable on screen. The leading space keeps that
# call out of history, and the line itself is added instead.
function __gill_submit --argument-names line
    history append -- $line
    set -g __gill_line $line
    set -l shown (string replace -a \n \n'# ' -- $line | string collect)
    commandline -r -- " __gill_run_line # $shown"
    commandline -f execute
end

function __gill_enter
    set -l line (commandline | string collect)
    if not commandline --paging-mode; and not commandline --search-mode
        and string match -qr '\S' -- $line
        and __gill_verdict $line
        __gill_submit $line
    else
        commandline -f execute
    end
end

function __gill_enter_bash
    set -l line (commandline | string collect)
    string match -qr '\S' -- $line
    and __gill_submit $line
end

# User bindings, which switching between default and vi key bindings keeps.
begin
    for mode in default insert
        bind -M $mode enter __gill_enter
        bind -M $mode ctrl-alt-b __gill_enter_bash
    end
end

function __gill_end --on-event fish_exit
    command gill end --session $__gill_session
end

# fish has no commands of these names, so defining them changes nothing for fish code.
function unset --description 'Run bash unset and keep the result'
    __gill_bash -- unset $argv
end

function declare --description 'Run bash declare and keep the result'
    __gill_bash -- declare $argv
end

function shopt --description 'Run bash shopt and keep the result'
    __gill_bash -- shopt $argv
end

function local --description 'Run bash local and keep the result'
    __gill_bash -- local $argv
end

function readonly --description 'Run bash readonly and keep the result'
    __gill_bash -- readonly $argv
end
