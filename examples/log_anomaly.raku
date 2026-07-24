#!/usr/bin/env silc
# Fetch log line → score anomaly → upsert metric
@version("1.0")

class LogEvent {
    has Str $.line;
    has num64 $.score;
}

class LogFetch is service {
    method fetch_line(Str $endpoint, :$timeout = 1000ms) {
        $endpoint
            ==> http::get()
            ==> schema::bind(LogEvent)
    }
}

class LogScorer is processor {
    method score(LogEvent $event) {
        $event.line
            ==> tensor::tokenize()
            ==> tensor::infer(:batch(8), :prefer<CPU>)
    }
}

class MetricSink is sink {
    method upsert(LogEvent $event) {
        $event
            ==> ipc::share_buffer()
            ==> store::upsert_primary()
    }
}
