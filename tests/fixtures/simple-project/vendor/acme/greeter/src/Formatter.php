<?php

namespace Acme\Greeter;

use Acme\Greeter\Hello;

interface FormatterInterface
{
    public function format(string $message): string;
}

class Formatter implements FormatterInterface
{
    private Hello $hello;

    public function __construct(Hello $hello)
    {
        $this->hello = $hello;
    }

    public function format(string $message): string
    {
        return strtoupper($message);
    }

    public function greetFormatted(string $name): string
    {
        $greeting = $this->hello->greet($name);
        return $this->format($greeting);
    }
}
