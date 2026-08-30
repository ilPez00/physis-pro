# Physis Monitor

An Android collector for a Physis engine you run yourself.

Your phone notices things you never write down: that there was a workout, that
it happened at the gym, that the Tuesday lesson did not happen this week. This
app sends those observations to your own engine, which decides what they meant.

## The split it is built on

The device **perceives**; the engine **means**. This app infers almost nothing —
steps are steps and a fix is a fix. Whether that was a workout, whether the gym
is a place you keep returning to, whether a weekly rhythm has started slipping:
all of that is `soma`, `topos` and `chronos` on the server, through
`POST /api/v1/monitor/ingest`.

That is why the app is small and why it has no database. Moving the inference
here would mean two implementations of what a morning was, and eventually two
answers.

## What it records

Nothing until you turn it on, one source at a time:

- **Steps** — walks and workouts
- **Places** — coordinates at most every five minutes, clustered into recurring
  places *on your server*
- **Speech** — that talking happened, roughly how long, roughly how many voices.
  **Never the audio.** See [PRIVACY.md](PRIVACY.md)
- **Battery** — how hard the day was on the phone

## Build

```sh
echo "sdk.dir=$ANDROID_HOME" > local.properties
./gradlew :app:assembleDebug
```

JDK 17+, Android SDK platform 35. No other setup.

## Dependencies

`androidx.core` and `androidx.appcompat`. That is the whole list. Networking is
`HttpURLConnection`, JSON is `org.json`, location is `LocationManager` — all
AOSP. **No Play Services, no Firebase, no analytics, no crash reporter.**

Not for purity: every dependency is something a store reviewer has to accept and
a user has to trust, and none of these would buy anything here.

## Releasing

See [RELEASE.md](RELEASE.md) — direct APK from git is the primary channel, Play
is secondary, and the data-safety answers are written out there.
