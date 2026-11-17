# `ultiwriter`

Client for the Vikant Ultimate Box embroidery card reader/writer

```
usage: vikant <command>
  info                         box dialect, card presence, flash ID/size
  read <addr> <len>            hexdump bytes from the card
  dump <file>                  read the whole card to a file
  ls [file]                    list Viking designs on the card (or in a dump)
  write <addr> <file> --yes    program bytes (target must be erased/blank)
  erase --yes                  chip-erase the card
  restore <file> --yes         erase, write the image, verify

embroidery files (pes pec dst exp jef vp3 pen; format from the extension):
  show <file> [--cmds]         summarise a design (--cmds: every command)
  convert <in> <out>           convert between formats
  export <dir> [fmt] [--from dump.bin]
                               save each card design as <dir>/NN.<fmt> (default vp3)
  image <out.bin> <file>...    build a Viking card image from design files
  import <file>... --yes       erase the card and write these designs to it
Addresses/lengths accept decimal or 0x-hex.
```
