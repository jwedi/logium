Problem statement:


The problem that logium use case that logium tries to solve is detecting known failure cases based on logs and alerting you when they happen.
An example is matchmaking failures for games. You have a client matchmaking and is expecting to join server. Matchmaking fails for the client and you get asked to identify why it didn’t join the expected server.
To figure this out you need to look at the logs of the client at the time it’s matchmaking and know the state of the server which you can deduce based on logs. If the server is in state X1, for example full when the client starts matchmaking then that’s a known failure case and you can deduce that matchmaking failed due to the server being full.
On the other hand if the server is in a different region than the client then that may also be a known failure case. Reading both logs manually, establishing the global order of events and deducing which failure case has been encountered can be very repetitive and time consuming.
Logium solves this problem by letting you create a catalog of log patterns and run them through multiple log files or log streams and instantly tell you if any of the patterns were encountered and where. This tells you which failure case you have encountered.

Logium components:
Source template:
Describes a certain log source and how to interpret its content. 
Describes how to read the timestamp.
Describes how to read the content of the file.
Describes the log line delimiter, i.e new line or something else.
Source:
A source of logs that can be loaded by Logium for analysis.
Is associated with a log template that tells Logium how to understand the log lines (like the timestamp and the log content), from the source.
Log line:
Contains a timestamp for when the log happened.
Contains a source identifier.
Contains the log content.
Log rule:
A “Match rule” is a regex rule that can match or not match a certain log line. 
A “Extraction rule” is a regex rule that extracts values out of a log line and exposes them as attributes.
A log rule can contain multiple “Match rules” and a setting determining if they should be evaluated as ”any” or “all” matches, i.e do all match rules have to match the log line for the log rule to match, or any of the match rules.
A log rule can contain multiple “Extraction rules” all of which are always evaluated on the matching log line. The extraction rules output values that can be used to visualise the state of the source at a certain point in time.
Also outputs the timestamp and the log line in case of a match.
Ruleset:
A named collection of rules that can be applied to a given source.
Log pattern:
A named sequence of log rules that identifies a specific failure case or “detection”. Each log rule can have a few qualifiers:
Ordering: 
Sequential: means that the log pattern should only be considered a match if the log rule comes after a match for the previous log rule in the sequence. Comes after means in terms of log timestamp.
Global: means that the log rule is considered a match no matter if the previous log rule matches or not.
Timeline:
Merged chronological view of all log rule matches and log pattern matches. Contains the timestamp and the log rule extracted value of all matched log rules. In the same chronological view shows whenever a log pattern is matched.
A view where all log rule matches and their extracted value is viewed in chronological order.
Also shows when log patterns match in the timeline.

Maybe log rule produces log state from extraction rules instead extraction rule specifies if extraced values / state are accumulated or replaced when matching the log rule again. Extraction rules can contain statically extracted values as well as parsed values. That way you can statically add something to the log state whenever the log rule matches. But additionally you can also remove/override something from the log state to clear it. Should also be able to fully clear the log state with some specific output. Maybe the extracted value can be parsed, static, or clear.
Log pattern are rules for log state, if the log state contains X, Y, Z in a certain order the pattern matches.
