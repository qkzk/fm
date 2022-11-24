# This file should be sourced

# [unix.stackexchange](https://unix.stackexchange.com/a/450752/515688)

# Depends on what you're going to do, another solution can be creating a function instead of a script.
#
# Example:
#
# Create a function in a file, let's say /home/aidin/my-cd-script:
#
# function my-cd() {
#   cd /to/my/path
# }
# Then include it in your bashrc or zshrc file:
#
# # Somewhere in rc file
# source /home/aidin/my-cd-script
# Now you can use it like a command:
# $ my-cd

# start fm and capture it's output
dest=$(/home/quentin/gclem/dev/rust/fm/target/debug/fm)

# if fm returned an output...
if  [[ ! -z $dest ]]
then
  # cd to it
  cd $dest;
fi

return 0
