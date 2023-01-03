# OODLES
It's not an acronym it's just very loud.

A single user program for writing in a twitter-thread-like way, but more powerful.

### 😴 This repository is archived, but might be worked on again in the future?; it's in hibernation

## What OODLES was supposed to be
I used to work on little programming projects and make a Twitter thread recording my progress. This was helpful to *me*, in documenting what I was doing and very much helping me think, and it seemed that some people liked reading them.

But I wanted more, right? I wanted to be able to edit the messages if I needed. And it would be nice if I could kind of transform it into a more "formal" *(for some definition of the word)* thing; some kind of article maybe.

But I wanted it to retain the original, "live" messages. So it was supposed to happen on Oodles, too. You would select some Oodle messages and be able to associate them with a paragraph or sentence or whatever. So there was always like a "link back" to the original.

There's a thing that happens sometimes today. Where people might livestream a game on Twitch or YouTube or something and then later cut it into a video. I don't think "highlights" is the right word, but it might be.

So it was supposed to be akin to that. The Oodle itself would be the like, "livestream recording" and the article-after-the-fact would e the edited, polished "highlights video".

But then my brain ran away from me. I wanted to be able to link all the messages and Oodles together at will, with ease. Make a sort of graph of things. If one Oodle related to another I wanted it to form a link, a connection, but it got hard.

And through this all I wanted to maintain a philosophy of "data first" or like "files first". Which is why Oodles are a sort of marked-up file and not just shoved into postgres, mysql, redis etc. It would certainly be easier doing that, but I like the more resilient nature of a file. If things fail, if Oodles ceases to exist, the data is still there. And pretty easy to parse! If everything fell down and you only had the dive, the Oodles themselves, you could still look at this data pretty much unchanged.