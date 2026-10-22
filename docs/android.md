# Draugr Time — Android companion (M13, later)

A minimal Android app. Core feature: a share-target inbox, like Freeform's.

## Share-target inbox (like Freeform boards)

Register the Android app as a share target (share sheet / `ACTION_SEND`), so any
app — Pinterest, a browser, a photo gallery — can share content to Draugr Time.

Shared items land in an **Inbox** first. Links show a preview (title, thumbnail,
URL); images and text are kept as-is. From the inbox the user moves each item
onto a **board** — the desktop freeform canvas (M12) — where captured links,
clips and notes sit side by side and can be organized later.

Open questions to resolve when this is scoped:

- How do boards sync between the desktop app and Android?
- What is captured per share (URL, text, image) and how is it stored?
