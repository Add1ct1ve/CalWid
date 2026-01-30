# CalWid - Calendar Widget

A minimal desktop calendar widget built with Tauri (Rust + WebView2) that displays your Google Calendar events and Google Tasks.

![Calendar Widget](https://img.shields.io/badge/Platform-Windows-blue) ![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)

## Features

- **24-hour calendar view** with scrollable week display
- **Google Calendar integration** - shows events from all your calendars
- **Google Tasks integration** - displays tasks from "My Tasks" lists
- **Complete tasks** directly from the widget
- **Frameless window** - clean, minimal design
- **Week number display** - shows ISO week number
- **Current time indicator** - red line showing current time
- **Keyboard navigation** - arrow keys to change weeks, Escape to close popups
- **Auto-refresh** - updates every 5 minutes
- **Caching** - instant startup with cached data

## Screenshots

The widget shows a week view with:
- Day headers with date numbers
- Time column on the left (00-23)
- Events positioned by time
- All-day events at the top of each day
- Tasks section at the bottom

## Prerequisites

- Windows 10/11 with WebView2 runtime
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/)
- Google Cloud Console project with Calendar and Tasks APIs enabled

## Setup

### 1. Clone the repository

```bash
git clone https://github.com/Add1ct1ve/CalWid.git
cd CalWid
```

### 2. Install dependencies

```bash
npm install
```

### 3. Set up Google API credentials

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select existing
3. Enable **Google Calendar API** and **Google Tasks API**
4. Go to **Credentials** → **Create Credentials** → **OAuth 2.0 Client ID**
5. Select **Desktop app** as application type
6. Download the JSON and save as `credentials.json` in the project root

The `credentials.json` should look like:
```json
{
  "installed": {
    "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
    "client_secret": "YOUR_CLIENT_SECRET",
    "redirect_uris": ["http://localhost"]
  }
}
```

### 4. Build and run

Development mode:
```bash
npm run tauri dev
```

Build release:
```bash
npm run tauri build
```

The executable will be at `src-tauri/target/release/calendar-widget.exe`

### 5. First run - OAuth authentication

On first run, your browser will open for Google authentication. Grant access to Calendar and Tasks. The token will be saved to `token.json` for future use.

## Autostart on Windows

To start the widget automatically on boot:

1. Copy `calendar-widget.exe` to your desired location
2. Create a VBS script in `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\`:

```vbs
Set WshShell = CreateObject("WScript.Shell")
WshShell.CurrentDirectory = "C:\path\to\calendar-widget-folder"
WshShell.Run """C:\path\to\calendar-widget.exe""", 0, False
```

## Usage

- **Drag** the header to move the window
- **Left/Right arrows** or **< >** buttons to navigate weeks
- **Click** an event to see details
- **Click** a task to mark it complete
- **X** button or close the window to exit

## Tech Stack

- **Frontend**: Vanilla HTML/CSS/JS
- **Backend**: Rust with Tauri 2.0
- **APIs**: Google Calendar API, Google Tasks API
- **Auth**: OAuth 2.0 with PKCE

## License

MIT
