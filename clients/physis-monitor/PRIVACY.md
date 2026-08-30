# Physis Monitor — what it records, and what it refuses to

This is the document the Play data-safety form is filled in from, and the one a
user should be able to read instead of trusting a checkbox.

## Where your data goes

**To your own server, and nowhere else.** You type in the address of a Physis
engine you run. The app has no backend, no analytics, no crash reporter and no
advertising SDK. There is no account, because there is nobody to have an
account with.

If you never set an address, the app records nothing and sends nothing.

## What is collected, when you switch it on

Every source is **off until you turn it on**, and each asks for its permission
at the moment you do.

| Source | What leaves the phone | What does not |
|---|---|---|
| Steps | number of steps and the minutes they cover | nothing else — the app cannot see *where* you walked from this |
| Places | latitude, longitude and the accuracy of the fix, at most every 5 minutes | no continuous trace; the engine clusters fixes into places you return to and discards the rest |
| Speech | that speech happened, for roughly how long, and a rough guess at whether more than one person was talking | **the audio itself, ever** |
| Battery | the charge percentage | nothing else |

## The microphone, specifically

The app never records audio.

That is a property of how it is built, not a promise about how it is operated.
`AudioProbe.kt` opens the microphone, computes three numbers from each ~62 ms
frame — loudness, zero-crossing rate, and whether that combination looks like
speech — and overwrites the buffer with the next frame. One buffer exists for
the lifetime of a listening window. The file contains no `File`, no
`OutputStream`, no `Uri` and no network call; there is nowhere for sound to go.

What reaches your engine is a sentence like *"someone was speaking for about
three minutes, probably more than one person"*. Not a recording. Not a
transcript. Not a keyword. The app cannot produce any of those, and adding one
would mean changing that file, which is where a reviewer — or you — should
look first.

**Why it is built this way.** Recording yourself is your business. A lesson, a
meeting or a call captures other people who did not agree to it, and in Italy
that engages art. 615-bis and 617 of the criminal code; under GDPR it leaves
the "purely personal activity" exemption as soon as it is stored. The legal
risk lives in *keeping the audio*. The thing you actually wanted — knowing that
Tuesday afternoon had a two-hour lesson in it — never needed the audio at all.

## What the app keeps on the phone

Your server address, your token, which sources you enabled, and the observations
that have not synced yet. Once the engine confirms it has them, they are
dropped. The phone is a sensor, not an archive — an archive on the phone is a
second copy of your life to lose with it.

## The status bar dot

While monitoring runs, Android shows a microphone or location indicator. That
indicator is correct and the app does not try to make it forgettable: the
notification says which sources are active, and it goes out when you stop.

## Turning it off

Stop the service from the app; uninstalling removes everything the app stored.
What your own engine already received is yours, on your own machine, and this
app has no way to reach it.
