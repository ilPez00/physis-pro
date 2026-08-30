# Releasing Physis Monitor

Two channels: **direct APK from git** (primary) and **Play** (secondary).
F-Droid is not a target — it requires every dependency to be free software —
but nothing here links against Play Services, so that road stays open if the
Play review turns out to be the harder one.

## Build

```sh
cd clients/physis-monitor
echo "sdk.dir=$ANDROID_HOME" > local.properties     # once
./gradlew :app:assembleDebug                        # ~3 MB, installable now
./gradlew :app:assembleRelease                      # needs signing, below
```

Requires JDK 17+ and an Android SDK with platform 35. No other setup: the
Gradle wrapper fetches its own distribution.

## Signing a release build

Generate a key once and keep it out of the repository:

```sh
keytool -genkeypair -v -keystore ~/.physis-release.jks \
  -alias physis -keyalg RSA -keysize 4096 -validity 10000
```

Then build with the credentials in the environment rather than in a file:

```sh
PHYSIS_KEYSTORE=~/.physis-release.jks \
PHYSIS_KEYSTORE_PASS=… PHYSIS_KEY_ALIAS=physis PHYSIS_KEY_PASS=… \
  ./gradlew :app:assembleRelease
```

*(The signing block is not in `build.gradle.kts` yet — add it when the first
real release is cut, reading those variables. It is left out deliberately: a
committed signing config is a committed invitation to commit a keystore.)*

## Direct distribution from git

Attach the signed APK to a GitHub release and publish its SHA-256 next to it:

```sh
sha256sum app/build/outputs/apk/release/app-release.apk
```

Users installing this way must allow installation from unknown sources. That is
a real friction and the right audience accepts it: anyone running a self-hosted
intelligence engine is already past that step.

Release builds are **not minified** on purpose (`isMinifyEnabled = false`), so
an APK published from git can be diffed against the source that claims to
produce it without a mapping file in the way.

## Play Store

Expect scrutiny on exactly the two permissions this app exists for. Answer from
the design rather than around it.

### Data safety form

| Question | Answer |
|---|---|
| Does your app collect or share user data? | **Collects**, does not share |
| Is data transferred off the device? | Yes — **to a server the user runs and names**. There is no developer-operated backend |
| Location — collected? | Yes, **approximate and precise**, when the user enables "Places" |
| Location — purpose | App functionality (the product is a record of the user's own day) |
| Audio — collected? | **No.** The app uses the microphone but transmits no audio, no transcript and no derived content beyond a duration and a voice-count estimate |
| Personal identifiers | None. No account, no email, no advertising ID, no device ID beyond a per-app `ANDROID_ID` fragment used to name the phone |
| Analytics / crash reporting | None. The app contains no analytics SDK |
| Is data encrypted in transit? | Over HTTPS when the user's engine address is HTTPS. Plain HTTP is permitted because a self-hosted engine on a LAN commonly has no certificate — this is stated in-app rather than hidden |
| Can users request deletion? | The data is on their own machine; the app holds nothing to delete beyond unsynced samples, which uninstalling removes |

### Background location declaration

Play requires a written justification and a demo video. The justification:

> The app records the user's own daily rhythm — which places they return to,
> and when — into a knowledge engine that the user themselves hosts. Location
> is sampled at most every five minutes, clustered into recurring places on the
> user's own server, and never transmitted to the developer, who operates no
> server. Background access is required because the value of the feature is
> precisely the hours when the user is not looking at their phone. The feature
> is off by default and is enabled explicitly in the app's first screen.

### Microphone disclosure

State plainly, and link `PRIVACY.md`:

> The microphone is used to detect **whether** speech is occurring and roughly
> how many voices, so the user's day can record that a conversation or a lesson
> took place. No audio is recorded, stored, transmitted or transcribed. Audio
> frames are analysed in memory and discarded within the same processing pass;
> the code has no file, stream or network path for audio (`AudioProbe.kt`).

### Likely review friction

- Background location almost always draws a follow-up. The demo video should
  show the toggle being turned **on by the user**, and the notification naming
  what is active.
- "Microphone but no audio collection" is an unusual combination and may be
  questioned. `AudioProbe.kt` is short and readable for exactly this reason —
  point at it.
- Do not add an analytics SDK "for release readiness". It would turn a
  no-data-shared declaration into a data-sharing one and lose the argument
  above.
