---
title: Serial consoles
description: Talk to routers, switches, microcontrollers and anything else with a UART — from the desktop or from a phone with a USB adapter.
---

## Desktop

**New host ▾ → Serial** (or <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>S</kbd>) lists the serial ports on the machine — `/dev/ttyUSB0`, `/dev/ttyACM0`, `COM3`, `/dev/cu.usbserial-…` — with the adapter's name where the system knows it. Pick one and the baud rate; the console opens as a terminal tab with the same scrollback, search, splits and logging as an SSH session.

![Serial connection dialog](/docs/img/desktop-serial.png)

**Advanced** holds the rest of the line settings, for the equipment that is not 8N1:

![Advanced serial settings](/docs/img/desktop-serial-advanced.png)

| Setting      | Values                                   | Note                                                                |
| ------------ | ---------------------------------------- | ------------------------------------------------------------------- |
| Baud rate    | 300 … 4 000 000, with the common presets | 9600 for most network gear, 115200 for Raspberry Pi and most boards |
| Data bits    | 5, 6, 7, 8                               |                                                                     |
| Parity       | None, Even, Odd, Mark, Space             |                                                                     |
| Stop bits    | 1, 1.5, 2                                |                                                                     |
| Flow control | None, RTS/CTS, XON/XOFF                  | Hardware flow control needs the lines wired; otherwise leave it off |

A serial console can be saved as a host so the settings are remembered, and it appears in the host list with a plug icon.

### Permissions

On Linux the user has to be in the `dialout` (Debian/Ubuntu) or `uucp` (Arch, Fedora) group: `sudo usermod -aG dialout $USER`, then log out and in. On macOS and Windows nothing is needed beyond the adapter's driver, and modern adapters need none.

## Android (USB-UART)

Connect a USB-to-serial adapter with an OTG cable, then open **Connections → Serial (USB)**. Termoso drives the adapter itself over the Android USB host API — no root, no vendor app. Supported chips:

- CDC-ACM (most Arduino boards, Raspberry Pi Pico, ESP32-S3/C3 native USB, many modern adapters);
- FTDI FT232/FT2232;
- Silicon Labs CP210x;
- WCH CH340/CH341.

Pick baud rate, data bits, parity, stop bits, flow control and the text encoding, and the console opens in the terminal with the usual key panel — the arrow gesture and the tmux row work the same. Plugging an adapter in offers to open it; unplugging it closes the session cleanly.

> [!NOTE]
> Android asks for permission to use the USB device the first time; tick **Always open** to skip the prompt for that adapter.

## Tips

- Nothing on screen after connecting: press <kbd>Enter</kbd> once — most devices only print a prompt when poked. Rubbish on screen: wrong baud rate.
- A device that resets when the port opens (Arduino, ESP boards) is reacting to DTR being asserted; that is normal and is how their bootloaders are triggered.
- Session recording works for serial consoles too, which makes a good capture of a boot log.
