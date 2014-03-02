## Play Tetris in the terminal!

![screen shot](https://raw.github.com/jankes/tetris1/master/tetris1.png)

### A simple Tetris clone implemented in Rust
(See http://www.rust-lang.org and https://github.com/mozilla/rust for more information on the Rust programming language)

### Supported Platforms

* It runs on my machine (Ubuntu 13.10) and will probably work on similar Linux distros.
* It may also compile/run on a Mac, but I haven't tested this.
* Windows is not supported.

### Compiling

The code currently compiles with rustc version 0.10-pre (c81b3fb 2014-03-01 19:44:37 -0800)

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
