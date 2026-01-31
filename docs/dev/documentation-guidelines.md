# Documentation Guidelines for Standout

Standout has a few uncommon bits, so it's worth going over them here as they affect documentation
The two ideas bellow; the lower level lib + framework structure and the continuum of adoption, value and understanding drive our guidelines..G


Standout is a full shell cli framework for complex, non interactive shell applications (otherwise the TUI ones have got you covered)
Pushing a full framework for shell apps is a hard ask. Between not seeing the value, the skepticism over reliability and locking and plain old habit, it's just a bad plan to push a all or nothing design. The core idea we want to move forward is: it's just too easy to mix shell specific parts inside and app logics  , like print statements, env var access etc.Before you know it, your application is all mixed up in logic with presentation. The first casualty is testing: testing becomes hard, and devs will either give up, or start integration testing everything, which is hard, error prone, brittle and requires you to parse and reverse engineer your logic that your formatting processed.


## Context

### 1. The Framework over Lover Level Libs 

Given that, if we can advance the idea that a strong separation between logic and presentation in shell programs is desirable and easy, that's a win. 
For that reason, we've structured the project in two layers: 
    - base creates: (i.e. standout-render, standout-dispatch), lower level libraries that can be used for a more focus task, and don't dictate architectures, designs. They are stand alone libs that you can use alone, or both, with or without the framework. 
    - standout: the full framework. Most of the functionality is provided by the lower level crates, while the framework will wire them together, cut the boilerplate and offer a stock configuration that is highly productive, feature rich and simple to setup.
- standout-dispatch:  a execution flow orchestrator that manages cli arg + logic and presentation handers + hooks and runs them in a structured way.
- standout-render: a full env, with template files, css styling, themes, hot reload during dev and more, to produce the best output effortlessly from your presentation formatting step.
- standout : the actual full framework

This design has two main objectives: 
    - Facilitate adoption: users can use one or both, and customize them to their respective needs, while having a clear message and value.
    - Improve the framework's design: building on top of distinct, stand alone creates ensures that we don't couple domains that shouldn't, that the foundation is flexible to cater to various use cases and so on.
    - Documentation and communication: since users might use one crate. both or the full framework, writing a single documentation that would cater for all possible permutations was impossible..
Hence, the two main pillars of standout are: 


### 2 The Guides vs Topics Styles

Again, asking developers to complete change their apps, with a subtle value proposition is a pipe dream.
However, adoption is more of a spectrum than a yes and on, and there is value in many points along the way. When you put these two together: 

- Most developers are reluctant to use frameworks for shell apps (either don't see the need, don't have the habit, likely both)
- We see value in a list of smaller steps towards the goal of shell apps that are easy to write and maintain.

Hence, our documentation style is designed walk-through a specific domain / task by a tutorial, real use case scenario that is shown in many small steps.  
Each step is valuable on it's own, hence, we , naturally, think the full list of steps provides optimal value, although it requires more commitment and work.

This format allows us: 
    - To explain the motivation and benefits at each step. 
    - To show how standout / crates can help you achieve there.

That is, this format allows users to engage with varying levels of buy in, while giving us an opportunity to show the value and motivation for each things.
This are the guides, and the quintessential example is ../guides/intro-to-standout.md . 

## Guidelines

1. General
    1.1. The core documentation should , match the code, and be in the crate it's in.  
    1.2. It should assume the stand alone crate as a lib usage, not presuppose standout the framework.
    1.3. However: callout boxes in relevant pages (for example over a configuration / glue code that the standout framework handles for it) is welcomed.
    1.4. Is it all about the why and how, now the what: 
        Modern IDES and tooling have become really good. Simply parroting an API is pointless, as users can do that on their IDES, much faster and without interruptions.
        The documentation is about exposing the problem that code solves, it's design. Designs are trade-offs, and it's often useful to go over what that allows and what it makes difficult.
        So the why is what problem is being solved? THe how is : the design, the blue print for this solution.
        We like to give users context so that understand how things connect, why certain some choices were made (when counter-intuitive or stray from mental mainstream.)

2. Guides: 
    2.1. Sequence of steps with increasing : value and commitment
    2.5. Each step should explain the why as needed.
    2.6 Always show the canonical recommend way to do something. (see bellow)
    2.7 Use callouts or mention more advanced options 
3. Topics: 
    While less verbose and deeper into the details than guides, topics also benefit from a more general introduction with the problems context, design etc

3. Canonical or Recommended APIs

    Standout strives for ergonomic, easy to use APIs. When possible, we love derive macros. This plays well with the idea that we are non intrusive on your app, and annotating it goes a long way.

    In guides we always showcase the recommend forms. Topics, we showcase and recommend them as well, but also cover the other options for users who need a particular behaviour 

    1. Annotation / declarative over imperative: i.e. derive macros vs imperative code.
    2. MiniJinja templates, css for styles
    3. File based presentation assets (templates, css )
    4. Semantic styling (two layer first style is about the semantics (what the item is) and it links a to a visual style)
    5. Core testing is done on the logic handlers
    6. Use as much as possible from the convention name matching (i.e. template names and handlers names)
    7. Symlik the full framework docs (docs) to the actual crates (<crate>/docs/) files
