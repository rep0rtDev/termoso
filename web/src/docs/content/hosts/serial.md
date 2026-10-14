---
title: Serial consoles
description: Talk to routers, switches, microcontrollers and anything else with a UART — from the desktop or from a phone with a USB adapter.
---

## Desktop

**Hosts → Serial** lists the serial ports on the machine. Pick one, set baud rate, data bits, parity, stop bits and flow control, and the console opens as a terminal tab with the same scrollback, search and logging as an SSH session.

## Android (USB-UART)

Connect a USB-to-serial adapter with an OTG cable, then open **Connections → Serial (USB)**. Termoso drives the adapter itself over the Android USB host API — no root, no vendor app. Supported chips:

- CDC-ACM (most Arduino boards, Raspberry Pi Pico, many modern adapters);
- FTDI FT232/FT2232;
- Silicon Labs CP210x;
- WCH CH340/CH341.

Pick baud rate, data bits, parity, stop bits, flow control and the text encoding, and the console opens in the terminal with the usual key panel. Plugging an adapter in offers to open it; unplugging it closes the session cleanly.

> [!NOTE]
> Android asks for permission to use the USB device the first time; tick **Always open** to skip the prompt for that adapter.
