# B3C security boundary

The webview may invoke only a bounded OCR action against an opaque/native-approved document reference. It may not pass absolute filesystem paths, OCR model paths, executable paths, URLs, database paths, output paths, arbitrary CLI arguments, or generic shell/process commands. OCR produces factual evidence only and has no authority to classify, post, reconcile, confirm matches, or determine tax treatment.
