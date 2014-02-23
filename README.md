# Play Tetris in the terminal!

## A simple Tetris clone implemented in Rust.

http://www.rust-lang.org
https://github.com/mozilla/rust

### Supported Platforms

* It runs on my machine (Ubuntu 13.10) and will probably work on similar Linux distros.
* It may also compile/run on a Mac, but I haven't tested this.
* Windows is not supported.

### Compiling

The code currently compiles with rustc version 0.10-pre (2ba0a8a 2014-02-22 11:41:48 -0800)

Assuming you have the Rust compiler installed, just run
    
    $ git clone https://github.com/jankes/tetris1
    $ cd tetris1
    $ rustc tetris1.rs

### How to Play

    # Show the help
    $ ./tetris1 --help

    # Just play the game
    $ ./tetris1
    
    # Play with a "double sized" display
    # (your terminal needs at least 100 columns for this to work)
    $ ./tetris1 --display=double
    
    # Show scores (stored in scores.json file in your current working directory)
    $ ./tetris1 --scores
    
- Left/right arrow keys move the falling piece left and right
- Up arrow rotates
- Down arrow "quick drops"
- Press any other key to quit

### State of the code

This is just a side project I made to play with the Rust programming language, and attempt to create a Tetris like game. It probably doesn't have the highest quality, most idiomatic Rust code, but it does work.
