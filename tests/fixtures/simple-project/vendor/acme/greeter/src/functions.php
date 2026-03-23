<?php

namespace Acme\Greeter;

function acme_greet(string $name): string
{
    $hello = new Hello();
    return $hello->greet($name);
}

function acme_version(): string
{
    return Hello::VERSION;
}
