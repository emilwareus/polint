# Teaching a Linter to Read JavaScript

*A narrated field guide to JavaScript call-graph construction. Written to be heard. 2026-06-09.*

---

Here is a question that sounds trivial and turns out to be one of the harder problems in
program analysis. You point a tool at a JavaScript file, you put your finger on a line
that calls a function, and you ask: when this runs, which function actually gets called?

Answer that question for every call site in a program and you have built a call graph. A
call graph is a map. The functions are places, and the calls are roads between them. Once
you have the map, a lot of valuable things become possible. You can follow a path from a
piece of untrusted input to a dangerous operation and warn that the path exists. You can
notice that a function has no roads leading into it and flag it as dead. You can answer
the question every engineer asks before a refactor, which is "if I change this, what else
moves?"

The reason this matters in practice, and the reason serious people have spent years on
it, is security. Modern applications are mostly other people's code. A small Node project
can pull in hundreds of packages, and any one of them might contain a known
vulnerability. But a vulnerability in a dependency only matters to you if your code can
actually reach it. The call graph is what tells you whether the dangerous function in
some transitive dependency is connected, by any path, to code you actually run. That is
the difference between a real alert and noise, and noise is what kills security tooling.

So the call graph is the foundation. And in JavaScript, the foundation is built on sand.

## Why JavaScript fights back

In a language like Java or Go, building the map is hard but the language helps you. Types
are written down. Methods belong to classes. The compiler has already resolved most of
who calls whom before you even start. You are mostly reconstructing structure that the
language made explicit.

JavaScript made almost none of it explicit. A function in JavaScript is just a value,
exactly like the number five or the string "hello." You can store it in a variable, drop
it into an array, hand it to another function as an argument, return it, attach it to an
object as a property, copy it from one object to another, and then call it through any of
those indirections. There is no type to pin it down, no declaration site that the call
site points back to. The value of a function flows through the program like water, and
your job is to figure out where it ends up.

There is a second, subtler difficulty, and it is philosophical. You cannot actually
answer the question perfectly. A perfectly correct, or "sound," analysis would have to
account for every possible behavior of the program, including things decided at runtime
that no static tool can know. JavaScript has features, like building a function out of a
string of code and evaluating it, that make full soundness either impossible or useless.
So every practical JavaScript call-graph analyzer makes a deliberate choice to be
unsound. It chooses to miss some real edges, or to occasionally invent one, in exchange
for being fast enough and precise enough to be worth running. The academic analyzer at
the center of this story says so directly in its own documentation: it models the
language, in its own words, intentionally not fully soundly. That is not a flaw. That is
the design. Holding that idea in your head — that we are deliberately building a useful
approximation, not a proof — changes how you think about every trade-off that follows.

This is the story of pushing one such analyzer, a static-analysis engine written in Rust,
measurably closer to the truth for JavaScript and TypeScript. Part of it is a travelogue
of a specific multi-day effort. Part of it is a guide to how this kind of work is done,
and done honestly. I am going to try not to round off the hard edges, because the point
is for you to come away actually understanding the machinery.

## The yardstick

You cannot improve what you cannot measure, so we need a source of truth to measure
against.

There is a research analyzer for JavaScript called Jelly, built by Anders Møller and
Oskar Haarklou Veileborg at Aarhus University. It is the product of more than a decade of
academic work on exactly this problem — its design draws on five separate research papers
with names like "modular call-graph construction for security scanning of Node
applications." Jelly is careful, it is slow, and it is accurate. For our purposes it
plays the role of the oracle. We run Jelly over a suite of JavaScript programs, we record
every call edge it finds, and that recorded set becomes the answer key.

Then we run our own engine on the same programs and compare, and every edge lands in one
of three buckets. A true positive is an edge we and the oracle agree on. A false positive
is an edge we reported that the oracle does not have — a call we hallucinated. A false
negative is an edge the oracle has that we missed — a real call we failed to find.

From those three counts come the two numbers that matter. Precision asks: of all the
edges we reported, what fraction were real? It measures how much we lie. Recall asks: of
all the real edges, what fraction did we find? It measures how much we miss. These two
pull against each other constantly. You can buy perfect recall by reporting every
conceivable edge, but your precision falls through the floor because most of those
guesses are wrong. You can buy perfect precision by only reporting the edges you are
absolutely sure of, but then you miss everything subtle and your recall collapses.

People fold the two into a single score called F1, the harmonic mean of precision and
recall. The harmonic mean is chosen on purpose because it is unforgiving. Unlike a plain
average, it stays low unless both numbers are high. You cannot hide terrible recall
behind beautiful precision. That property is exactly why it is the headline.

When this work began, the engine sat at an F1 of about seventy-five percent. Precision
was already excellent, around ninety-four percent. Recall was the laggard, around
sixty-three percent. The whole shape of the problem lives in those two numbers. We were
not lying much. We were missing a lot. So the entire job was to find more true edges
without spending down the precision we had banked. Every decision from here serves that
constraint.

## Two ways to build the map

There are, broadly, two philosophies for constructing a JavaScript call graph, and the
gap between them is the most important idea in this whole piece. Everything else is
detail hanging off this distinction.

The first philosophy is the one our engine mostly uses. Call it the recognizer approach.
You walk the program's syntax tree, and for each specific shape of code you know how to
handle, you have a hand-written rule that says, in this exact situation, the answer is
this. There is a rule for a direct call to a named function. A rule for calling a method
on an object you can see being built. A rule for a function handed to an array's forEach.
A rule for the callback attached to a promise. Each rule reads from and writes to a set
of bookkeeping maps — which variable currently holds which function, which object has
which properties, and so on. The recognizer approach is precise, it is fast, and you can
reason about one rule at a time, which makes it pleasant to work on. Its weakness is
structural. The rules only combine where you have explicitly wired them together. Every
new shape of code is a new, separate piece of logic, and the language has an unbounded
supply of shapes. You are, forever, playing whack-a-mole.

The second philosophy is the one Jelly uses, and it is worth understanding precisely,
because it is also where this story ends up pointing. Jelly does inclusion-based
points-to analysis, the style invented by Lars Ole Andersen in the nineteen-nineties.
Instead of matching shapes of code, you model the program as a giant system of
constraints over abstract values.

Let me make that concrete, because the vocabulary is the lesson. Every place a value can
be created — every function literal, every object literal, every array — becomes a token.
A token is an abstract stand-in that means "a value born at this spot in the code." Jelly
has function tokens, object tokens tagged by what kind of thing they are, whether array
or map or set or promise, native tokens for built-in objects, and special tokens for
values that arrive from libraries it has chosen not to look inside. Then, every place a
value can live becomes a cell, which the literature calls a constraint variable. A
variable is a cell. The return slot of a function is a cell. The "this" of a function is
a cell. The "arguments" of a function is a cell. And — this is the part that gives the
analysis its precision — each individual property of each individual object is its own
cell. That last property is called field sensitivity, and it means Jelly does not lump
all the dot-foo accesses in a program together. It keeps "the foo property of the object
born on line ten" separate from "the foo property of the object born on line twenty."

With tokens and cells in hand, you translate the whole program into inclusion
constraints, which are just statements about which cells must contain which tokens. A
function literal puts its token into a cell. An assignment says the tokens in one cell are
a subset of the tokens in another. Reading a property says the property's cell flows into
the result. And a call site says something richer: for every function token that flows
into the position being called, connect that call's arguments into the function's
parameter cells, and connect the function's return cell back out to the call's result.

Then you solve. You take all of those constraints and you push tokens through the cells,
following the subset relationships, over and over, until a full pass changes nothing.
That settled state is called a fixpoint, and it is your answer. In practice Jelly does
this with a worklist — a queue of cells that just gained new tokens and need to have those
tokens pushed to their neighbors — and it does some genuinely clever engineering to keep
it fast, like detecting cycles in the constraint graph and collapsing them, because
otherwise the same tokens chase each other around forever.

The reason this approach is so powerful is that it composes for nothing. You never write
a rule for "a function returned from a factory, then attached to an object, then read back
out and called." That pattern is not a special case. It is just tokens flowing from cell
to cell to cell until they arrive, and the call constraint fires when they get there. The
recognizer approach has to hand-build every such path. The points-to approach gets all
paths from the same handful of constraint types. The price is that it is a heavier machine
to build, and that controlling its precision — making sure it does not smear every value
into a vague cloud of maybes — takes real care. Jelly, notably, mostly does not use the
expensive precision techniques like distinguishing calls by their context; it stays
deliberately simple and instead bounds how far indirect reasoning is allowed to run.

Hold these two pictures side by side. The entire arc of what follows is the tension
between them. The recognizer approach took us a long way, cheaply. But every win got
smaller and harder to find, and by the end the evidence was unambiguous that the
points-to approach is the only way to go meaningfully further.

## A subtlety that powers everything: reachability

One more idea before the work, and it is the one that trips people up.

Jelly only reports calls from functions that are actually reachable from the program's
entry point. It analyzes the whole program, pulling in each file as the code requires it,
but when it is done, it computes which functions can actually be reached by following call
edges from the entry, and a function that nothing ever calls contributes no edges at all.
From the oracle's point of view, unreachable code has no call graph, because unreachable
code never runs.

Our engine, by contrast, naively analyzes every function body it can find, reachable or
not. So to compare fairly against the oracle, we have to play by the oracle's rule: we
filter our reported edges down to only those whose calling function we can connect back
to an entry point. This is reachability pruning, and it has a consequence that feels
unfair the first time you meet it. We can resolve an edge perfectly correctly and still
have it counted as a miss, purely because we could not prove the function it lives in is
reachable.

That coupling — recall is secretly tied to reachability — is the hidden engine behind one
of the largest wins in this story, and also behind its most instructive bug. The question
"which function runs here?" turns out to be welded to a second question: "does this
function run at all?"

## The crucible: one real Express app

The test suite holds seventy-six programs. Seventy-five are small, hand-written fixtures,
each one poking at a single JavaScript feature. The seventy-sixth is different. It is a
complete, real web application built on Express, the most popular web framework in the
Node world. With its dependencies installed it drags in around eighty source files and
roughly twenty third-party packages, and all by itself it accounted for more missed edges
than any other program in the suite. If you want to feel, in your bones, why JavaScript
call graphs are hard, you sit with this one program until you can recite it.

The application's own code is almost insultingly short. It asks for Express, it creates an
app, it registers one route handler for the home page, and it starts the server listening
on a port. Four lines of plain intent. And to know which functions those four lines
actually call, you have to understand a small mountain of machinery hidden inside the
framework. What makes it a perfect teaching case is that the machinery is not exotic. It
is ordinary JavaScript, used the way real libraries use it. And, worth knowing: even the
oracle has no special knowledge of Express. Jelly does not contain an Express model. It
resolves this app through the same general points-to machinery it applies to everything
else. The framework's behavior is not built in anywhere. It genuinely has to be derived.

Walk through it slowly.

The first line asks for the Express library, and the thing that comes back is not an
object. It is a function, named create-application. So when the app then calls Express as
if it were a function, it is really calling create-application. To know that, our engine
has to understand that a module can export a function, and that calling the imported name
reaches across the file boundary into that other file. That is the first hurdle:
cross-file resolution of an exported value.

Now look at what create-application does. It builds a brand-new local function — that
function is the app — and then it does something peculiar. It calls a helper imported from
a package named, literally, merge-descriptors, and that helper copies every property from
a prototype object onto the app. The prototype is itself imported from another file inside
Express. So the app's methods — the things like "use this middleware," "handle this
request," "listen on this port" — are not written on the app at all. They are copied onto
it, at runtime, by a helper function. To know the app even has a listen method, our engine
has to model that copy. Second hurdle: dynamic property copying from one object onto
another.

It gets stranger. The HTTP verb methods — get, post, put, delete, and the rest — are not
written out individually. Inside the framework there is a list of verb names, and the code
loops over that list, and for each name it attaches a function to the app under that name.
The get method and the post method are literally the same function, installed under
different keys by a loop. To know that calling the app's get method reaches that function,
our engine has to understand that a loop wrote a value under a key computed at runtime.
Third hurdle: a computed-key property write driven by a loop variable.

And finally, all of these methods are hung off an object that is itself the module's
exports, reached through a local nickname. The framework file begins, in effect, by saying
"let app be the exports, be the module's exports, be a fresh empty object," chaining the
assignments together, and then it attaches every method to that local name "app." So when
another file imports this module, it has to see all those methods as the module's exported
shape, even though they were written through a local variable that merely aliases the
exports. Fourth hurdle: property writes to a local alias of the module exports.

Now the cruel part, the part that had defeated an earlier attempt. None of these four
mechanisms pays off on its own. Build the cross-file return value but not the property
copy, and the app comes back empty. Build the property copy but not the loop write, and
the verb methods are missing. Build the loop write but fail to carry the app across the
file boundary, and nothing connects to anything. The four hurdles form a chain, and a
chain with one link missing holds no weight. This is exactly why, earlier in the project,
someone had built cross-file return values by themselves, measured the benchmark, found it
had moved by precisely zero edges, and correctly thrown the work away. That is not a
failure of the idea. It is the chain telling you the truth. When mechanisms interlock, you
must build them together and judge them together. That became the governing rule of the
entire effort.

## Building the chain

So we built all four links, in a careful order, with a safety net under each. Each one is
a small idea worth understanding on its own.

The first link is the export alias. When the code chains "let app be the exports be the
module exports be a fresh object," we now recognize that the local name "app" is standing
in for the module's exported object, and from then on, every property written onto app
gets folded into the module's exported shape. In practice this meant detecting that
chained-assignment pattern, seeding an empty object under the name so later writes have
somewhere to land, and merging that object into the module's public summary once the file
is fully walked. Small and self-contained, but without it the framework module exports
nothing useful.

The second link is my favorite, because it is a genuinely new idea in this codebase. It
is the wildcard property. Remember the loop that installs the verb methods under computed
names. The loop variable could be any of the verb strings, and we cannot and should not
try to enumerate them. So instead of recording "get maps to this function, post maps to
this function," and so on, we record something more honest: any property read on this
object, if it has no explicit entry of its own, resolves to this function. We added a
wildcard lane to the object model — a catch-all bucket of function values. When we later
ask for the app's get method and find no explicit get, we fall through to the wildcard,
and there is the verb function. The important discipline is that we only fall through when
there is no explicit entry. The app's listen method, which is written out by hand, keeps
resolving to listen and is never polluted by the verb wildcard. This is the mirror image
of an older feature that read properties through a loop variable. This is the writing side
of the same coin.

The third link teaches the engine about the merge-descriptors helper. We recognize when a
local name is bound to that specific imported helper, and then a call to it, copying a
source object onto a target, merges the source's entire shape — including that new
wildcard lane — into the target. We reused machinery that already existed for the standard
library's object-assign operation. The merge-descriptors helper is the same idea wearing a
library's clothes.

The fourth link is the heaviest, and it is the one that had failed twice before in
isolation: the cross-file return summary. The idea is to compute, for every function in
every file, a summary of what it returns, both the functions it returns and the shape of
the object it returns. We do this in a dedicated pass that runs after the module summaries
have already settled, so that by the time we evaluate create-application's body, the
prototype it imports is already known, the property-copy helper does its work, and the
app that comes back carries the full merged shape. Then, when the application calls
Express, we look up create-application by its identity, fetch its return summary, and hand
the calling code an app that finally knows all of its methods.

With this fourth link in place, and only with all four together, the chain connected.
Calling Express resolved to create-application. The app's get method resolved to the verb
function. The app's listen method resolved to listen. Four lines of intent, finally
mapped onto the real functions beneath them. The raw gain was modest — a handful of edges
— but it was the right handful, it cost zero false positives, and it was the platform for
everything after.

## The most instructive bug

Now the chapter I would keep if I could keep only one, because it taught the deepest
lesson and it turns on that reachability idea from earlier.

Recall that our recall is coupled to reachability. There is a beautiful consequence buried
in the Express app. Deep inside the framework's own files there are something like seventy
call edges that our engine already resolves correctly, but that get thrown away as
unreachable, because we could not connect those interior functions back to the program's
entry. The entry chain was broken, so the whole interior was floating free, and every
correct edge inside it was being discarded.

We called this double leverage. If you connect the entry chain — which we just did — then
resolving it does not only add the chain's own edges. It also un-prunes all those interior
edges that were correct the whole time but orphaned. One fix, paid twice.

To actually collect the second payment, the framework's interior methods had to resolve
their own internal calls. The verb function, for instance, calls "this dot lazyrouter" and
"this dot set." Those resolve only if, when we walk the verb function's body, the word
"this" is bound to the app object. So we built a mechanism to walk the body of a method
attached to an object with "this" pointed at that object — exactly analogous to how we
already walked the methods of classes. We aimed it at the export-alias objects, walked
every method hanging off the app, and let their internal this-calls resolve.

And it worked, at first, beautifully. Recall jumped by nearly fifty edges. But precision
fell off a cliff, dropping below the line we had promised never to cross. We had traded
clean accuracy for raw volume. So we stopped and asked the only question worth asking in
that moment: where are these new false positives coming from?

The answer was subtle, and then, once seen, obvious. Our reachability check had a clever
shortcut baked into it. It treated any function that emitted one of these value-flow edges
as reached — as a starting point of the reachability search — on the theory that we only
ever walk a function's body when something actually calls it. That theory had been true.
But our new mechanism walked method bodies speculatively. It walked every method on the
object, including the dozen Express methods this particular app never touches. Each of
those dead methods, once walked, emitted edges, and each therefore crowned itself a
reachability root. Dead code was promoting itself to live code and dragging its edges into
the answer as false positives.

The fix is a small, satisfying thing. We gave the edges from this speculative walk a
distinct label — a new category that says, in effect: I am a real edge, and I still count
if my function turns out to be reachable, but I am not allowed to make my function
reachable on my own. Edges with that label still flow through the reachability search, so
a method genuinely reached through the real call chain still un-prunes everything it
calls. But a dead method walked speculatively no longer seeds itself as a starting point.
It stays pruned, which is exactly correct.

With that one distinction, the picture transformed. The false positives vanished entirely,
back to where they had started. And we kept a solid chunk of the recall — the methods
reachable through the genuine entry chain un-pruned their interiors, around nineteen
edges' worth, while the dead methods stayed silent. Precision held. That is the whole game
in miniature. A recall mechanism is only as good as the precision discipline wrapped
around it, and the right precision fix is not always "report fewer edges." Sometimes it is
"be more careful about what you allow to count as alive."

## The same idea, widened

Once an idea is good, the move is to ask where else it lives, and walking method bodies
with "this" bound turned out to live in several places.

Classic JavaScript, the kind written before the language grew a class keyword, builds
objects with constructor functions and prototypes. You write a function, you hang methods
off its prototype, and inside those methods you say "this dot something" to reach a
sibling. One of the heaviest dependencies in the Express app, a library for parsing IP
addresses, is written almost entirely this way. We extended the same walk to prototype
methods, with the same careful reachability label so that prototype methods nobody ever
constructs stay pruned. Along the way we fixed a real gap: the engine had only been
treating a function as a constructor when it saw an explicit class keyword, so plain old
prototype constructors were invisible to it. Teaching it to register them the moment it
saw a method assigned to a prototype lit up a whole family of library code. This was the
first change in the run that helped both halves of the suite at once — the real app and
the small fixtures.

The next idea came from the test-framework fixtures. Some programs in the suite use a
testing library's describe and it functions — the "describe this suite, it should do this"
pattern any test author knows. The interesting thing about describe and it is that they
call the function you give them. The framework invokes your callback. That is the same
shape as a promise's then handler: a function called by machinery rather than by a visible
call in your own code. So we taught the engine that these well-known entry points invoke
their callback arguments, walked those bodies, and let the calls inside them resolve and
become reachable.

And here the second instructive thing happened. That change, by itself, moved nothing.
Zero edges. Byte-for-byte identical output. The reason was a humbling little gap. Inside
the test bodies, the actual calls were buried inside expressions our value-flow walk
simply never descended into. A line that asserts the type of the result of calling a
library function equals the string "number" hides the interesting call several layers
deep, inside a comparison, inside an argument, inside another call. Our walk knew how to
descend into call arguments and assignments, but not into the two sides of a comparison,
or the operand of a "type of," or the branches of a conditional, or the slots of a
template string. The buried call was never even visited, so of course nothing resolved.

The fix was unglamorous and broadly useful: make the walk complete. Teach it to descend
into binary expressions, unary expressions, logical operators, conditionals, template
literals, and member expressions, so a call hiding anywhere in an expression gets reached.
With that completeness in place, the framework model finally paid off. The lesson is one
every analysis engineer learns sooner or later. A clever feature that moves nothing is
very often not wrong. It is blocked, by some boring gap upstream, and finding that gap is
the actual work.

The last win in the run was computed property keys on "this." Code sometimes writes a
property whose name is built at runtime — concatenating two strings, or reading a name out
of a constant. When that write targets "this," the engine needs to evaluate the key down
to a real name and record that the object now has that method. We already had the
string-evaluation machinery. We just had to use it on the writing side of a this
assignment, in the two places it mattered. Small and clean.

When the dust settled, the run had moved the engine from an F1 of about seventy-five
percent to about seventy-seven percent — a gain of nearly two points — and it had done so
with the false-positive count completely unchanged across every single iteration. Recall
climbed from roughly sixty-three to sixty-five percent. Precision held just above
ninety-four and a half. Not one new lie. Thirty-nine real edges recovered. Every step
reversible, every step logged.

## How to do this without fooling yourself

I want to dwell on method, because the how of this work matters as much as the what, and
it is the part most likely to transfer to whatever analysis you find yourself improving.

Test the smallest thing first. Before a mechanism is allowed anywhere near the big
benchmark, it gets a tiny hand-written program that exercises exactly the shape it is
meant to handle and asserts exactly the edge it is meant to find. These tests run in
milliseconds, they pin the mechanism's intent against later refactors, and they catch the
embarrassing mistakes early — including the time a test "failed" not because the feature
was broken but because the test was searching the source text for a function call and
accidentally matched the function's declaration instead.

Measure the combination, and hold the line on precision. There was a hard floor: precision
was not allowed to drop below what we had banked. Every mechanism ran against the full
benchmark, and the result was kept only if true positives went up and false positives did
not. This sounds obvious and it has teeth. It is the rule that forced the
reachability-label fix instead of letting us ship a fifty-edge recall gain that secretly
shipped fifty new lies.

Revert cleanly when a change earns nothing. This is the hardest one emotionally. Twice
near the end of the run, I built a mechanism, gated it with a passing test, ran the
benchmark, and got back output identical to before. One treated constructor functions that
assign to "this" as classes. The other resolved positional access into a function's
arguments object. Both passed their unit tests. Both were real, correct features. And both
moved the benchmark by exactly zero, because the specific edges they targeted were already
being resolved by machinery that existed. The temptation in that moment is enormous — the
code works, the test is green, surely it is fine to keep. But carrying a feature that
improves nothing is how a codebase quietly rots. Dead complexity is a tax on everyone who
reads the code after you. So both went back, cleanly, to the exact prior state. Deleting
your own correct, working, useless code is one of the more mature habits an engineer can
build.

And keep an honest log. Every iteration was recorded — the change, the counts before and
after, the precision and recall and F1, the runtime, and a content hash of the output so
that "no change" could be proven rather than asserted. Including the reverts. The project
history is candid about three earlier dead ends, and they are the most useful entries in
it. One was an attempt to model a prototype-reading built-in that introduced a false
positive, because our flattened object model could not represent a prototype being
replaced rather than merged — the kind of distinction that needs a more expressive state
than we have. One was that lonely cross-file return summary that moved nothing on its own,
which is the very thing the export chain later reintroduced, this time paired with the
three mechanisms it actually needs. And one was an early version of the class-body walk
that gained four edges but cost five, because of a mismatch between which node our engine
blames a constructor's calls on and which node the oracle blames. A log that records only
victories is a marketing document, not a research record, and it will lie to the next
person who reads it — quite possibly you, three months from now.

## What is actually left

Which brings us to the honest accounting. What remains is a little over five hundred
missed edges and about fifty false positives, and the shape of what remains is the most
useful thing in this guide, because it tells you where the recognizer approach ends.

The gap divides into two worlds. About half lives in that single real Express app. The
other half is scattered across the small fixtures.

In the Express app, the missed edges are now mostly inside the dependency tree — the
internal call graphs of the twenty-odd packages it pulls in — and they cluster around a
few structural blockers worth naming.

The biggest single blocker is the response object. When the app registers its route
handler, the framework does not call that handler right away. It stores it, and calls it
much later, when a real web request arrives, passing it a request object and a response
object the framework built internally. So the handler's call to send a response can only
resolve if two things are true. First, the engine has to understand that the framework
will eventually invoke the stored handler at all. Second, it has to know that the
handler's second parameter carries the response object, with all its methods. Until that
send call resolves, the entire response module's interior stays unreachable and pruned —
something like twenty-plus edges sitting just out of reach behind one missing link. And
the link is deeply specific to this one framework. Modeling "Express calls route handlers
with request and response objects shaped like so" is a recognizer for one library's
private conventions, and it carries real risk of inventing false edges if you apply it
carelessly. High value, high risk.

The second blocker is instance typing across methods. Inside the framework, one method
stashes a router object on "this," and a later method pulls it back out and calls methods
on it. To resolve those, the engine would have to track that a particular property holds a
particular class instance, and carry that fact from the method that wrote it to the method
that reads it. The flat collection of bookkeeping maps the recognizer approach uses does
not naturally carry that kind of cross-method instance type. You would be hand-threading
it, method by method, and it would be fragile.

The third group is the ordinary library code — the IP parser, the deprecation helper, the
debug logger, the HTTP-error factory. They miss for the same reasons the small fixtures
miss: prototype objects whose instances are built deep in code we never reach, curried
factories that return functions that return functions, wrappers generated at runtime by
evaluating a string of code, functions used as objects with "this" pointing back at
themselves. It is the long tail of ordinary JavaScript, now at real-library scale.

The other world is the small fixtures, and here the single largest category — something
like a hundred and thirty missed edges — is what I would call pure value-flow depth. Each
one is, in essence, one more hop. A function returns an object, the object lands in a
variable, a property of it is read into another variable, that is passed to a function,
that function returns it, and finally it is called. Every individual hop is trivial. There
is nothing exotic in any single step. But our recognizer has to have explicitly threaded
together every combination of hops, and there are simply too many combinations. This is
the category a points-to fixpoint closes for free, because in that model the value just
flows from cell to cell to cell until it arrives, with no rule required for the particular
path it took. A hundred and thirty edges that are agony for recognizers and trivial for a
fixpoint — that is the single loudest argument in the whole dataset for changing the
architecture.

The rest of the fixture tail is a grab-bag of individually-hard mechanisms: the precise
semantics of the arguments object after it has been reassigned, calling a function through
arguments-dot-callee, parenthesized and curried call targets that are notoriously
finicky, a constructor that overrides its own return value, classes exported as a module's
default and built in another file, super calls in exotic positions, the finer points of
async and generator values. Each is real, each is a few edges, each needs its own bespoke
recognizer. Diminishing returns, steeply.

And a word on the leftover false positives, because they are instructive. Almost all of
them come from arrays, and the reason is a choice the oracle made. Jelly models arrays
without distinguishing their indices — it deliberately blurs all of an array's elements
together, so that the moment you write to an array at a computed position, every element
becomes uncertain. Our engine, in some of these cases, is more precise than the oracle. It
correctly works out that the first element is a specific function. But because the oracle
blurred them, our precise answer does not match its blurry one, and it is scored as a false
positive. "Fixing" this would mean deliberately making our analysis worse to match the
oracle's coarseness — and worse, the change would risk the dozens of array edges we
already get right. So we left it. It is a sharp reminder that when you measure against an
oracle, you are bound to the oracle's choices, including its imprecisions, and chasing the
metric can occasionally mean degrading the analysis.

## The endgame

Step back from the individual cases and one conclusion comes into focus. The recognizer
approach has been mined nearly to exhaustion. The proof is not an argument; it is those
two reverts. When two carefully built, correct, tested mechanisms in a row produce zero
change because the edges they targeted were already handled, you are no longer extending a
frontier. You are filling in a region that is already covered, or bouncing off cases that
need something fundamentally different.

What is left needs the second philosophy, and we can now describe the destination
precisely, because Jelly is the existence proof. The next real step is to build a small,
private points-to engine inside our analyzer — a value-token heap. You introduce tokens
for the kinds of values that flow through a JavaScript program: function tokens, object
tokens, array tokens, promise tokens, module tokens. You introduce cells for the places
those tokens can live: variables, object properties kept separate per object, function
parameters, return slots, the "this" binding, the "arguments" binding. You translate the
program into inclusion constraints over that world — a literal puts a token in a cell, an
assignment makes one cell a subset of another, a property write puts tokens into an
object's property cell, a call connects arguments to parameters and the return back out.
And then you turn the crank, propagating tokens through cells with a worklist until a full
pass changes nothing.

This is the endgame because almost everything on the remaining list collapses into it. The
hundred-and-thirty-edge depth problem evaporates, because depth is just tokens taking more
hops and a fixpoint does not care how many. The dependency-tree internals come along,
because they are the same general flow at scale. Prototype chains, constructor returns,
cross-module classes — most of them become "feed the heap the right starting constraints"
rather than "write another recognizer." It is, in effect, building a scoped-down version
of exactly what the oracle does, which is the strongest possible evidence that it would
work.

The catch is real: this is an architecture, not an afternoon's patch, with its own
correctness and precision and performance concerns, built carefully over a meaningful
stretch of time. And that is precisely why the responsible move at the end of this run was
to stop — to bank a clean, reversible, well-documented two-point gain with zero precision
cost, and to name the next thing as a deliberate project rather than start a major rewrite
halfway and leave it in a worse state than a clean stopping point. Knowing when a mature
approach is finished, and saying so plainly, is part of the craft too.

## The shape of the thing

If you take nothing else from this, take the shape of it. A call graph answers a simple
question — which function runs here — and JavaScript makes that question genuinely hard,
because functions are ordinary values slipping through variables and properties and
arguments with no type system to pin them down, and because the only useful analyses are
deliberately, knowingly incomplete.

There are two ways to build the map. Recognizers match specific shapes of code; they are
precise and fast and easy to reason about, and they do not compose, so you add rules
forever. Points-to analysis models values as tokens flowing through cells to a fixpoint;
it composes for free and it scales to the long tail, at the cost of being a heavier
machine and demanding care to keep precise. The first gets you surprisingly far. The
second is where you go when the first runs dry.

And the work itself has a discipline. Build interlocking mechanisms together, because a
chain with a missing link holds nothing. Remember that recall and reachability are
secretly the same problem, so that a recall fix does not quietly corrupt your sense of
what runs. Gate every change behind a small test, judge it against the real benchmark,
hold a hard line on precision, throw away your own correct code when it earns nothing, and
keep a log honest enough to record the dead ends. Do that, and you can improve an analysis
steadily for days without waking up one morning to find you have been trading away
accuracy the entire time.

The numbers moved — thirty-nine real edges, two points of F1, not one new false positive —
and, just as valuable, the work drew a clear, evidence-backed line where this approach ends
and the next one has to begin. In analysis, as in most engineering, that line is worth
nearly as much as the gain.
