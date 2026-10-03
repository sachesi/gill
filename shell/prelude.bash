# Runs one command for gill in a fresh, non-interactive bash: restore the state the
# previous command saved, run the command at top level, save the state again.
#
# Arguments: the snapshot to restore, the directory to save into, then `line LINE` or
# `argv COMMAND [ARG...]`. Everything here calls builtins through `builtin`, so
# functions the user defines with the same names do not change it.

__gill_snapshot=$1 __gill_dir=$2 __gill_mode=$3
builtin shift 3
if [[ $__gill_mode == line ]]; then
    __gill_line=$1
    builtin set --
fi

builtin shopt -s expand_aliases

# Whatever exists before the snapshot is restored belongs to bash itself or came from
# fish's environment, and stays out of the next snapshot.
builtin declare -A __gill_known
while IFS= builtin read -r __gill_entry; do
    __gill_known[$__gill_entry]=1
done < <(builtin compgen -v; builtin shopt -p; builtin set +o)
# Bash sets this inside the save function itself.
__gill_known[FUNCNAME]=1

__gill_save() {
    builtin trap - EXIT
    builtin local __gill_name

    # Shell options first, so that extglob and the like are on before the functions
    # that need them are parsed. Functions come before aliases: their bodies are
    # stored with aliases already expanded and must not be expanded twice.
    # errexit, nounset, verbose and xtrace are only switched on right before the next
    # command, so they neither stop the restore nor trace it.
    {
        while IFS= builtin read -r __gill_name; do
            [[ -v __gill_known[$__gill_name] ]] || builtin printf '%s\n' "$__gill_name"
        done < <(builtin shopt -p; builtin set +o)

        while IFS= builtin read -r __gill_name; do
            [[ $__gill_name == __gill_* || -v __gill_known[$__gill_name] ]] && continue
            # Exported variables belong to fish; restoring them would undo changes made
            # there since.
            [[ ${!__gill_name@a} == *x* ]] && continue
            builtin printf 'builtin declare -p %s >/dev/null 2>&1 ||\n' "$__gill_name"
            builtin declare -p -- "$__gill_name"
        done < <(builtin compgen -v)

        while IFS= builtin read -r __gill_name; do
            [[ $__gill_name == __gill_* ]] || builtin declare -f -- "$__gill_name"
        done < <(builtin compgen -A function)

        builtin alias -p

        builtin printf '__gill_options=%s\n' "${__gill_flags//[!euvx]/}"
    } >|"$__gill_dir/snapshot.next" || builtin return "$__gill_status"

    {
        builtin printf '%s\0' "$PWD"
        while IFS= builtin read -r __gill_name; do
            [[ ${!__gill_name@a} == *x* && -v $__gill_name ]] || continue
            # Bash never puts arrays in the environment.
            [[ ${!__gill_name@a} == *[aA]* ]] && continue
            builtin printf '%s\0%s\0' "$__gill_name" "${!__gill_name}"
        done < <(builtin compgen -e)
        builtin printf '\0'
    } >|"$__gill_dir/environ.next"

    builtin return "$__gill_status"
}

# Both the trap, which covers `exit`, and the end of this script, which covers a
# command that replaced the trap, save the same way: record `$?` and the options while
# they are still the command's, silently, then turn off the options that would trace
# the save or end it early.
builtin trap -- '{ __gill_status=$? __gill_flags=$-; builtin set +euvx; } 2>/dev/null; __gill_save' EXIT

__gill_options=
if [[ -f $__gill_snapshot ]]; then
    builtin source -- "$__gill_snapshot"
fi

if [[ $__gill_mode == line ]]; then
    [[ -z $__gill_options ]] || builtin set "-$__gill_options"
    builtin eval -- "$__gill_line"
else
    [[ -z $__gill_options ]] || builtin set "-$__gill_options"
    "$@"
fi
{ __gill_status=$? __gill_flags=$-; builtin set +euvx; } 2>/dev/null
__gill_save
