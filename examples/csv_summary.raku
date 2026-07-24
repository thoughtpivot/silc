#!/usr/bin/env silc
# CSV path → column summary → store report key
@version("1.0")

class CsvJob {
    has Str $.path;
    has Str $.report_key;
}

class CsvUpload is service {
    method accept_path(Str $path, :$timeout = 2000ms) {
        $path
            ==> http::post()
            ==> schema::bind(CsvJob)
    }
}

class ColumnSummarizer is processor {
    method summarize(CsvJob $job) {
        $job.path
            ==> pandas::read_csv()
            ==> numpy::describe()
    }
}

class ReportSink is sink {
    method save(CsvJob $job) {
        $job
            ==> store::upsert_primary()
    }
}
