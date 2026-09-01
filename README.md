prefset
=======

Synchronize macOS preferences with one config file

Quick Start
-----------

```
$ brew install bahamas10/tap/prefset
$ prefset init
$ vim ~/.config/prefset/config.toml
...
$ prefset apply --dry-run
$ prefset apply
```

Example Config
--------------

The single config is super simple and written in toml:

```toml
[defaults.NSGlobalDomain]

# disable spell correction
NSAutomaticSpellingCorrectionEnabled = false

# speed up keyboard repeat rate
KeyRepeat = 2

[defaults."com.apple.dock"]
orientation = "left"
autohide = true

[defaults."com.apple.finder"]
ShowStatusBar = true

# ... and more ...
```

Check the [assets/](/assets) directory for more config examples.

Example Run
-----------

When you first run the program it will detect if the config file is found, and
prompt you to create it if not:

```
$ prefset
to get started, create and test a minimal config with:

    prefset init
    $EDITOR ~/.config/prefset/config.toml
    prefset apply --dry-run

run `prefset -h` or `prefset apply -h` to see more options

$ prefset init
created: ~/.config/prefset/config.toml

look over this file, modify it to your liking, then run:

    prefset apply --dry-run

to get started.
```

After modifying the config to your liking you can apply it with `prefset apply`.
Also, consider running with `--dry-run` first to see what will be changed
without actually changing anything.

```
$ prefset apply
[defaults.NSGlobalDomain]
    → NSAutomaticSpellingCorrectionEnabled: true -> false
    → KeyRepeat: 3 -> 2

[defaults."com.apple.dock"]
    → orientation: "right" -> "left"
    → autohide: false -> true

[defaults."com.apple.finder"]
    → ShowStatusBar: false -> true

- updated 5/5 preferences

----------------------------------------

2 process(es) need to relaunch: Dock, Finder

> relaunching the services helps these changes take effect.
> this will not log you out, but affected UI may briefly disappear.
> will run: /usr/bin/killall Dock Finder

relaunch services now? [y/N]: y
relaunched Dock, Finder

✓ done
```

And that's it!

If you run it again you'll see that no changes were made because everything was
already set.

```
$ prefset apply
[defaults.NSGlobalDomain]
    ✓ NSAutomaticSpellingCorrectionEnabled = false
    ✓ KeyRepeat = 2

[defaults."com.apple.dock"]
    ✓ orientation = "left"
    ✓ autohide = true

[defaults."com.apple.finder"]
    ✓ ShowStatusBar = true

- updated 0/5 preferences
✓ done
```

Usage
-----

```
$ prefset -h
Synchronize macOS preferences with one config file

Usage: prefset [OPTIONS] [COMMAND]

Commands:
  init    Create a starter config file
  check   Exit successfully if all configured preferences are set
  apply   Apply configured preferences
  help    Print this message or the help of the given subcommand(s)

Options:
      --config <PATH>  Config file [default: ~/.config/prefset/config.toml]
  -h, --help           Print help
  -V, --version        Print version

Examples:
  prefset check               Check if preferences are set
  prefset apply --dry-run     See what changes would be made
  prefset apply               Actually set your preferences
```

License
-------

MIT License
