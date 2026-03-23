<?php

namespace Acme\Greeter;

class Hello
{
    public const VERSION = '1.0.0';

    public function greet(string $name): string
    {
        return "Hello, {$name}!";
    }

    public static function create(): self
    {
        return new self();
    }
}
