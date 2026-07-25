#!/usr/bin/env silc
# Minimal AI chatbot: polished web UI, local Llama, and SQLite history.
@version("1.0")

subset NonEmpty of Str where { .chars > 0 }

class ChatTurn {
    has UUID $.id;
    has NonEmpty $.prompt;
    has Str $.reply;
    has Str $.model;
}

class AiChatbot is service {
    method listen(:$port = 18091) {
        ChatTurn
            ==> ui::web(:port(18091), :route("/"))
    }
}

class SmartAssistant is processor {
    has Str $.model_ref = "llama3.2-1b";

    method complete(ChatTurn $turn) {
        $turn.prompt
            ==> llm::complete(:model($.model_ref))
    }
}

class ChatHistory is sink is storage(SQLite) {
    method persist(ChatTurn $turn) {
        $turn
            ==> ipc::publish()
            ==> store::sqlite(:table(chat_turns))
            ==> store::commit()
    }
}
