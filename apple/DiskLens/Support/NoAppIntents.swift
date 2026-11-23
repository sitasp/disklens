import AppIntents

/// DiskLens does not define App Intents. Importing the framework is what
/// stops Xcode 27 from warning, on every build, that metadata extraction was
/// skipped because AppIntents.framework was not linked. There is no intent,
/// shortcut, or query in this file.
enum DiskLensAppIntentsLink {}
